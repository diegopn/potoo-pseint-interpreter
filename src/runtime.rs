use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::rc::Rc;
use std::time::Duration;

use crate::builtins::{self, Random, decimal, display_number};
use crate::check::Types;
use crate::parser::{
    Expr, Expression, Line, Name, Op, Passage, Place, Program, Statement, Type, Value,
};
use crate::{CallSite, Diagnostic, Host, Position, Step, Variable};

pub(crate) fn execute<'a>(
    program: &Program<'a>,
    types: &Types<'a>,
    input: &mut impl BufRead,
    output: &mut impl Write,
    host: &mut impl Host,
) -> Result<(), Diagnostic> {
    let random = Random::new(host.random_seed());
    let mut runtime = Runtime {
        program,
        types,
        input,
        output,
        host,
        random,
        line: String::new(),
        routine: program.main.name.0,
        call_depth: 0,
    };
    runtime.block(
        &mut seed_frame(&types[program.main.name.0]),
        &program.main.body,
    )
}

type Frame<'a> = HashMap<&'a str, Address>;

struct Slot {
    ty: Option<Type>,
    declared: bool,
    scalar: Value,
    used: bool,
    array: Option<Array>,
}

struct Array {
    dimensions: Vec<i32>,
    values: HashMap<Vec<i32>, Value>,
}

#[derive(Clone)]
struct Address {
    slot: Rc<RefCell<Slot>>,
    index: Option<Vec<i32>>,
}

impl Address {
    fn new(ty: Option<Type>, scalar: Value) -> Self {
        let slot = Rc::new(RefCell::new(Slot {
            ty,
            scalar,
            declared: false,
            used: false,
            array: None,
        }));
        Self { slot, index: None }
    }

    fn is_array(&self) -> bool {
        self.index.is_none() && self.slot.borrow().array.is_some()
    }

    fn ty(&self) -> Option<Type> {
        self.slot.borrow().ty
    }

    fn get(&self) -> Value {
        let mut slot = self.slot.borrow_mut();
        slot.used = true;
        let value = match &self.index {
            Some(index) => slot
                .array
                .as_ref()
                .and_then(|a| a.values.get(index))
                .unwrap_or(&Value::Empty),
            None => &slot.scalar,
        };
        match value {
            Value::Empty => slot.ty.map_or(Value::Empty, Type::default_value),
            Value::Input(text) => match slot.ty {
                Some(Type::Real | Type::Integer) => Value::Number(decimal(text).unwrap_or(0.0)),
                Some(Type::Logic) => Value::Logic(text == "VERDADERO"),
                Some(Type::Text) => Value::Text(text.clone()),
                None => value.clone(),
            },
            value => value.clone(),
        }
    }

    fn set(&self, value: Value, at: Position) -> Result<(), Diagnostic> {
        self.store(value, at, true)
    }

    fn store(&self, value: Value, at: Position, check_integer: bool) -> Result<(), Diagnostic> {
        let mut slot = self.slot.borrow_mut();
        if let Some(ty) = slot.ty {
            if !ty.accepts(&value) {
                if ty == Type::Integer && matches!(value, Value::Number(_)) {
                    if check_integer {
                        return Err(at.error(
                            "No coinciden los tipos, el valor a asignar debe ser un entero.",
                        ));
                    }
                } else {
                    return Err(at.error("No coinciden los tipos."));
                }
            }
        } else {
            let previous = if let Some(index) = &self.index {
                slot.array
                    .as_ref()
                    .and_then(|array| array.values.get(index))
            } else {
                Some(&slot.scalar)
            };
            if let Some(ty) = value.ty()
                && previous.is_some_and(|previous| {
                    matches!(previous, Value::Input(_)) && !ty.accepts(previous)
                })
            {
                return Err(at.error("No coinciden los tipos."));
            }
            slot.ty = value.ty();
        }
        slot.used = true;
        if let Some(index) = &self.index {
            slot.array
                .as_mut()
                .expect("indexed slot has an array")
                .values
                .insert(index.clone(), value);
        } else {
            slot.scalar = value;
        }
        Ok(())
    }
}

fn seed_frame<'a>(types: &HashMap<&'a str, Type>) -> Frame<'a> {
    types
        .iter()
        .map(|(&name, &ty)| (name, Address::new(Some(ty), Value::Empty)))
        .collect()
}

struct Runtime<'a, 'run, R, W, H> {
    program: &'run Program<'a>,
    types: &'run Types<'a>,
    input: &'run mut R,
    output: &'run mut W,
    host: &'run mut H,
    random: Random,
    line: String,
    routine: &'a str,
    call_depth: usize,
}

impl<'a, R: BufRead, W: Write, H: Host> Runtime<'a, '_, R, W, H> {
    fn block(&mut self, frame: &mut Frame<'a>, statements: &[Line<'a>]) -> Result<(), Diagnostic> {
        for Line(statement, at) in statements {
            self.on_line(frame, *at)?;
            match statement {
                Statement::Define(names, ty) => {
                    for &name in names {
                        let address = Self::slot(frame, name);
                        let mut slot = address.slot.borrow_mut();
                        if slot.declared || !matches!(slot.scalar, Value::Empty) {
                            return Err(name
                                .1
                                .error(format!("La variable ({}) ya estaba definida.", name.0)));
                        }
                        slot.ty = Some(*ty);
                        slot.declared = true;
                    }
                }
                Statement::Dimension(arrays) => {
                    for Place(name, dims) in arrays {
                        let dims = dims.as_ref().expect("dimensions required by parser");
                        let mut dimensions = Vec::with_capacity(dims.len());
                        for expr in dims {
                            let dim = self.evaluate(frame, expr)?.as_number(expr.1)? as i32;
                            if dim <= 0 {
                                return Err(expr.1.error("Las dimensiones deben ser mayores a 0."));
                            }
                            dimensions.push(dim);
                        }
                        let address = Self::slot(frame, *name);
                        let mut slot = address.slot.borrow_mut();
                        if slot.used || slot.array.is_some() {
                            return Err(name.1.error("Identificador en uso."));
                        }
                        slot.array = Some(Array {
                            dimensions,
                            values: HashMap::new(),
                        });
                    }
                }
                Statement::Assign(place, expr) => {
                    let address = self.address(frame, place, false)?;
                    let value = self.evaluate(frame, expr)?;
                    address.set(value, place.0.1)?;
                }
                Statement::Read(places) => {
                    for place in places {
                        let address = self.address(frame, place, false)?;
                        let value = self.read(address.ty(), place.0.1)?;
                        address.set(value, place.0.1)?;
                    }
                }
                Statement::Write(expressions, newline) => {
                    for expr in expressions {
                        let value = self.evaluate(frame, expr)?.display();
                        self.output
                            .write_all(value.as_bytes())
                            .and_then(|_| self.output.flush())
                            .map_err(|err| expr.1.error(format!("cannot write output: {err}")))?;
                    }
                    if *newline {
                        self.output
                            .write_all(b"\n")
                            .map_err(|err| at.error(format!("cannot write output: {err}")))?;
                    }
                    self.flush(*at)?;
                }
                Statement::Call(place) => {
                    self.call(frame, place)?;
                }
                Statement::If(expr, yes, no) => {
                    let body = if self.condition(frame, expr)? {
                        yes
                    } else {
                        no
                    };
                    self.block(frame, body)?;
                }
                Statement::While(expr, body) => {
                    while self.condition(frame, expr)? {
                        self.block(frame, body)?;
                        self.on_line(frame, *at)?;
                    }
                }
                Statement::Repeat(body, expr, while_true) => loop {
                    self.block(frame, body)?;
                    self.on_line(frame, expr.1)?;
                    if self.condition(frame, expr)? != *while_true {
                        break;
                    }
                },
                Statement::For(name, first, last, step, body) => {
                    let start = self.evaluate(frame, first)?.as_number(first.1)?;
                    let stop = self.evaluate(frame, last)?.as_number(last.1)?;
                    let step = match step {
                        Some(expr) => self.evaluate(frame, expr)?.as_number(expr.1)?,
                        None if start > stop => -1.0,
                        None => 1.0,
                    };
                    let address = self.address(frame, &Place(*name, None), false)?;
                    address.store(Value::Number(start), name.1, false)?;
                    loop {
                        let counter = address.get().as_number(name.1)?;
                        if if step >= 0.0 {
                            counter > stop
                        } else {
                            counter < stop
                        } {
                            break;
                        }
                        self.block(frame, body)?;
                        self.on_line(frame, *at)?;
                        let counter = address.get().as_number(name.1)?;
                        address.store(Value::Number(counter + step), name.1, false)?;
                    }
                }
                Statement::ForEach(name, array, body) => {
                    let element = self.address(frame, &Place(*name, None), false)?;
                    let address = Self::slot(frame, *array);
                    if !address.is_array() {
                        return Err(array
                            .1
                            .error(format!("La variable ({}) no es un arreglo.", array.0)));
                    }
                    let slot = address.slot;
                    let bounds = slot
                        .borrow()
                        .array
                        .as_ref()
                        .map(|array| array.dimensions.clone())
                        .ok_or_else(|| {
                            array
                                .1
                                .error(format!("La variable ({}) no es un arreglo.", array.0))
                        })?;
                    let mut index = vec![1; bounds.len()];
                    loop {
                        let entry = Address {
                            slot: slot.clone(),
                            index: Some(index.clone()),
                        };
                        element.set(entry.get(), name.1)?;
                        self.block(frame, body)?;
                        entry.set(element.get(), name.1)?;
                        let Some(last) =
                            index.iter().zip(&bounds).rposition(|(i, bound)| i < bound)
                        else {
                            break;
                        };
                        index[last] += 1;
                        index[last + 1..].fill(1);
                        self.on_line(frame, *at)?;
                    }
                    let mut slot = element.slot.borrow_mut();
                    if let Some(index) = &element.index {
                        slot.array
                            .as_mut()
                            .expect("indexed slot has an array")
                            .values
                            .remove(index);
                    } else {
                        slot.scalar = Value::Empty;
                    }
                }
                Statement::Switch(selector, cases) => {
                    let selected = self.evaluate(frame, selector)?;
                    if matches!(selected, Value::Logic(_)) {
                        return Err(selector.1.error(
                            "La expresión del SEGUN debe ser de tipo numérica o caracter.",
                        ));
                    }
                    'cases: for (labels, body) in cases {
                        for label in labels {
                            self.on_line(frame, label.1)?;
                            self.evaluate(frame, label)?;
                            let left = self.evaluate(frame, selector)?;
                            let right = self.evaluate(frame, label)?;
                            if binary(Op::Eq, left, right, label.1)?.as_bool(label.1)? {
                                self.block(frame, body)?;
                                break 'cases;
                            }
                        }
                        if labels.is_empty() {
                            self.block(frame, body)?;
                            break;
                        }
                    }
                }
                Statement::Clear => {
                    self.host
                        .clear_screen(self.output)
                        .map_err(|err| at.error(err.to_string()))?;
                }
                Statement::WaitKey => {
                    self.flush(*at)?;
                    self.host
                        .wait_for_key(self.input)
                        .map_err(|err| at.error(err.to_string()))?;
                }
                Statement::Wait(expr, factor) => {
                    let millis = self.evaluate(frame, expr)?.as_number(expr.1)? as i32;
                    if millis > 0 {
                        self.host
                            .wait(Duration::from_millis(millis as u64 * factor))
                            .map_err(|err| at.error(err.to_string()))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn on_line(&mut self, frame: &Frame<'a>, at: Position) -> Result<(), Diagnostic> {
        let inspect = |name: &str, subscript: Option<&[i32]>| {
            let address = frame.get(name.to_uppercase().as_str())?;
            let slot = address.slot.borrow();
            let (value, dimensions) = if let Some(array) = &slot.array {
                if let Some(index) = address.index.as_deref().or(subscript) {
                    if index.len() != array.dimensions.len()
                        || index
                            .iter()
                            .zip(&array.dimensions)
                            .any(|(index, end)| !(1..=*end).contains(index))
                    {
                        return None;
                    }
                    (array.values.get(index), Vec::new())
                } else {
                    (None, array.dimensions.clone())
                }
            } else {
                if subscript.is_some() {
                    return None;
                }
                (Some(&slot.scalar), Vec::new())
            };
            let value = value
                .filter(|v| !matches!(v, Value::Empty))
                .map(Value::display);
            Some(Variable { value, dimensions })
        };
        let step = Step {
            line: at.line,
            column: at.column,
            routine: self.routine,
            call_depth: self.call_depth,
            inspect: &inspect,
        };
        self.host
            .on_line(&step)
            .map_err(|err| at.error(err.to_string()))
    }

    fn slot(frame: &mut Frame<'a>, Name(name, _): Name<'a>) -> Address {
        frame
            .entry(name)
            .or_insert_with(|| Address::new(None, Value::Empty))
            .clone()
    }

    fn address(
        &mut self,
        frame: &mut Frame<'a>,
        Place(name, indexes): &Place<'a>,
        pass_array: bool,
    ) -> Result<Address, Diagnostic> {
        let mut address = Self::slot(frame, *name);
        let Some(indexes) = indexes else {
            if !pass_array && address.is_array() {
                return Err(name
                    .1
                    .error(format!("Faltan subíndices para el arreglo ({}).", name.0)));
            }
            return Ok(address);
        };
        if !address.is_array() {
            return Err(name
                .1
                .error(format!("La variable ({}) no es un arreglo.", name.0)));
        }
        let bounds = address
            .slot
            .borrow()
            .array
            .as_ref()
            .expect("array exists")
            .dimensions
            .clone();
        if indexes.len() != bounds.len() {
            return Err(name.1.error(format!(
                "Cantidad de índices incorrecta para el arreglo ({}).",
                name.0
            )));
        }
        let mut index = Vec::with_capacity(indexes.len());
        for (expr, upper) in indexes.iter().zip(bounds) {
            let value = self.evaluate(frame, expr)?.as_number(expr.1)? as i32;
            if value < 1 || value > upper {
                return Err(expr
                    .1
                    .error(format!("Subíndice ({value}) fuera de rango (1...{upper}).")));
            }
            index.push(value);
        }
        address.index = Some(index);
        Ok(address)
    }

    fn flush(&mut self, at: Position) -> Result<(), Diagnostic> {
        self.output
            .flush()
            .map_err(|err| at.error(format!("cannot flush output: {err}")))
    }

    fn read(&mut self, ty: Option<Type>, at: Position) -> Result<Value, Diagnostic> {
        self.output
            .write_all(b"> ")
            .map_err(|err| at.error(format!("cannot write prompt: {err}")))?;
        self.flush(at)?;
        self.line.clear();
        if self
            .host
            .read_line(self.input, self.output, &mut self.line)
            .map_err(|err| at.error(format!("cannot read input: {err}")))?
            == 0
        {
            return Err(at.error("Sin entradas disponibles."));
        }
        let line = self.line.trim_end_matches(['\r', '\n']);
        let upper = line.to_ascii_uppercase();
        let text = if matches!(upper.as_str(), "VERDADERO" | "FALSO") {
            &upper
        } else {
            line
        };
        let bad_type = || at.error("No coinciden los tipos.");
        match ty {
            Some(Type::Logic) => match upper.as_str() {
                "VERDADERO" | "V" | "1" => Ok(Value::Logic(true)),
                "FALSO" | "F" | "0" | "" => Ok(Value::Logic(false)),
                _ => Err(bad_type()),
            },
            Some(ty @ (Type::Integer | Type::Real)) => {
                if ty == Type::Integer && line.contains('.') {
                    return Err(
                        at.error("No coinciden los tipos, el valor ingresado debe ser un entero.")
                    );
                }
                decimal(line).map(Value::Number).ok_or_else(bad_type)
            }
            Some(Type::Text) => Ok(Value::Text(text.to_owned())),
            None => Ok(Value::Input(text.to_owned())),
        }
    }

    fn condition(&mut self, frame: &mut Frame<'a>, expr: &Expr<'a>) -> Result<bool, Diagnostic> {
        self.evaluate(frame, expr)?.as_bool(expr.1)
    }

    fn evaluate(
        &mut self,
        frame: &mut Frame<'a>,
        Expr(expr, at): &Expr<'a>,
    ) -> Result<Value, Diagnostic> {
        Ok(match expr {
            Expression::Literal(value) => value.clone(),
            Expression::Variable(place) => {
                if builtins::signature(place.0.0).is_some()
                    || self
                        .program
                        .routines
                        .iter()
                        .any(|routine| routine.name.0 == place.0.0)
                {
                    self.call(frame, place)?
                } else {
                    self.address(frame, place, false)?.get()
                }
            }
            Expression::IsInteger(operand) => {
                let left = self.evaluate(frame, operand)?.as_number(*at)?;
                let right = self.evaluate(frame, operand)?.as_number(*at)?;
                Value::Logic((left - f64::from(builtins::trunc(right))).abs() < 1e-11)
            }
            Expression::Unary(op, operand) => {
                let value = self.evaluate(frame, operand)?;
                match op {
                    Op::Not => Value::Logic(!value.as_bool(*at)?),
                    Op::Plus => Value::Number(value.as_number(*at)?),
                    Op::Minus => Value::Number(-value.as_number(*at)?),
                    _ => unreachable!("invalid unary operator"),
                }
            }
            Expression::Binary(Op::And, left, right) => {
                Value::Logic(self.condition(frame, left)? && self.condition(frame, right)?)
            }
            Expression::Binary(Op::Or, left, right) => {
                Value::Logic(self.condition(frame, left)? || self.condition(frame, right)?)
            }
            Expression::Binary(op, left, right) => {
                let left = self.evaluate(frame, left)?;
                let right = self.evaluate(frame, right)?;
                binary(*op, left, right, *at)?
            }
        })
    }

    fn call(
        &mut self,
        frame: &mut Frame<'a>,
        Place(Name(name, at), args): &Place<'a>,
    ) -> Result<Value, Diagnostic> {
        let args = args.as_deref().unwrap_or(&[]);
        if let Some(signature) = builtins::signature(name) {
            let mut values = Vec::with_capacity(args.len());
            for (arg, ty) in args.iter().zip(signature.params) {
                let value = self.evaluate(frame, arg)?;
                if !(match ty {
                    Type::Integer => value.as_number(arg.1).is_ok(),
                    ty => ty.accepts(&value),
                }) {
                    return Err(arg.1.error("Tipo de dato incorrecto en el argumento."));
                }
                values.push(value);
            }
            return builtins::evaluate(name, &values, *at, &mut self.random);
        }
        let routine = self
            .program
            .routines
            .iter()
            .find(|routine| routine.name.0 == *name)
            .ok_or_else(|| at.error(format!("Subproceso no definido ({name}).")))?;
        let mut locals = seed_frame(&self.types[name]);
        for (arg, (param, passage)) in args.iter().zip(&routine.params) {
            let place = match &arg.0 {
                Expression::Variable(place)
                    if builtins::signature(place.0.0).is_none()
                        && !self.program.routines.iter().any(|r| r.name.0 == place.0.0) =>
                {
                    Some(place)
                }
                _ => None,
            };
            let is_array = place
                .is_some_and(|place| place.1.is_none() && Self::slot(frame, place.0).is_array());
            let actual = if is_array || *passage == Passage::Reference {
                if *passage == Passage::Value {
                    return Err(arg
                        .1
                        .error("Los arreglos solo pueden pasarse por referencia."));
                }
                let reference = place.ok_or_else(|| {
                    arg.1
                        .error("No puede utilizar una expresión en un pasaje por referencia.")
                })?;
                self.address(frame, reference, true)?
            } else {
                let value = self.evaluate(frame, arg)?;
                let ty = locals
                    .get(param.0)
                    .and_then(Address::ty)
                    .or_else(|| value.ty());
                if ty.is_some_and(|ty| !ty.accepts(&value)) {
                    return Err(arg.1.error("Tipo de dato incorrecto en el argumento."));
                }
                Address::new(ty, value)
            };
            locals.insert(param.0, actual);
        }
        let caller = std::mem::replace(&mut self.routine, name);
        self.call_depth += 1;
        let result = self.block(&mut locals, &routine.body);
        self.call_depth -= 1;
        self.routine = caller;
        result.map_err(|mut diagnostic| {
            diagnostic.backtrace.push(CallSite {
                routine: (*name).to_owned(),
                line: at.line,
                column: at.column,
            });
            diagnostic
        })?;
        Ok(routine
            .result
            .map_or(Value::Empty, |name| Self::slot(&mut locals, name).get()))
    }
}

impl Type {
    pub(crate) fn default_value(self) -> Value {
        match self {
            Self::Integer | Self::Real => Value::Number(0.0),
            Self::Logic => Value::Logic(false),
            Self::Text => Value::Text(String::new()),
        }
    }

    fn accepts(self, value: &Value) -> bool {
        match (self, value) {
            (Self::Integer, Value::Number(n)) => {
                n.fract() == 0.0 && (*n >= f64::from(i32::MIN) && *n <= f64::from(i32::MAX))
            }
            (Self::Real, Value::Number(_))
            | (Self::Logic, Value::Logic(_))
            | (Self::Text, Value::Text(_))
            | (_, Value::Empty) => true,
            (Self::Integer, Value::Input(text)) => !text.contains('.') && decimal(text).is_some(),
            (Self::Real, Value::Input(text)) => decimal(text).is_some(),
            (Self::Logic, Value::Input(text)) => {
                matches!(text.as_str(), "VERDADERO" | "FALSO" | "")
            }
            (Self::Text, Value::Input(_)) => true,
            _ => false,
        }
    }
}

impl Value {
    fn ty(&self) -> Option<Type> {
        match self {
            Self::Number(_) => Some(Type::Real),
            Self::Logic(_) => Some(Type::Logic),
            Self::Text(_) => Some(Type::Text),
            Self::Input(text)
                if !text.is_empty()
                    && decimal(text).is_none()
                    && !matches!(text.as_str(), "VERDADERO" | "FALSO") =>
            {
                Some(Type::Text)
            }
            Self::Empty | Self::Input(_) => None,
        }
    }

    fn display(&self) -> String {
        match self {
            Self::Number(value) => display_number(*value),
            Self::Logic(true) => String::from("VERDADERO"),
            Self::Logic(false) => String::from("FALSO"),
            Self::Text(text) | Self::Input(text) => text.clone(),
            Self::Empty => String::new(),
        }
    }

    pub(crate) fn as_bool(&self, at: Position) -> Result<bool, Diagnostic> {
        match self {
            Self::Logic(value) => Ok(*value),
            Self::Input(text) if matches!(text.as_str(), "VERDADERO" | "FALSO" | "") => {
                Ok(text == "VERDADERO")
            }
            Self::Empty => Ok(false),
            _ => Err(at.error("Se esperaba un valor lógico.")),
        }
    }

    pub(crate) fn as_number(&self, at: Position) -> Result<f64, Diagnostic> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::Input(text) => {
                decimal(text).ok_or_else(|| at.error("Se esperaba un valor numérico."))
            }
            Self::Empty => Ok(0.0),
            _ => Err(at.error("Se esperaba un valor numérico.")),
        }
    }

    pub(crate) fn as_text(&self, at: Position) -> Result<&str, Diagnostic> {
        match self {
            Self::Text(value) | Self::Input(value) => Ok(value),
            Self::Empty => Ok(""),
            _ => Err(at.error("Se esperaba una cadena de caracteres.")),
        }
    }
}

fn binary(op: Op, left: Value, right: Value, at: Position) -> Result<Value, Diagnostic> {
    use Op::*;
    use Value::*;
    let ty = match op {
        Minus | Times | Divide | Power | Mod => Type::Real,
        _ => left.ty().or_else(|| right.ty()).unwrap_or_else(|| {
            if Type::Real.accepts(&left) && Type::Real.accepts(&right) {
                Type::Real
            } else if matches!(op, Eq | Ne)
                && Type::Logic.accepts(&left)
                && Type::Logic.accepts(&right)
            {
                Type::Logic
            } else {
                Type::Text
            }
        }),
    };
    let typed = |value: Value| match value {
        Empty | Input(_) => match ty {
            Type::Real | Type::Integer => value.as_number(at).map(Number),
            Type::Logic => value.as_bool(at).map(Logic),
            Type::Text => value.as_text(at).map(|text| Text(text.to_owned())),
        },
        value => Ok(value),
    };
    let left = typed(left)?;
    let right = typed(right)?;
    match (left, right) {
        (Number(left), Number(right)) => {
            let epsilon = 1e-11;
            let result = match op {
                Eq => return Ok(Logic((left - right).abs() < epsilon)),
                Ne => return Ok(Logic((left - right).abs() >= epsilon)),
                Lt => return Ok(Logic(left + epsilon < right)),
                Le => return Ok(Logic(left - epsilon <= right)),
                Gt => return Ok(Logic(left - epsilon > right)),
                Ge => return Ok(Logic(left + epsilon >= right)),
                Plus => left + right,
                Minus => left - right,
                Times => left * right,
                Divide if right == 0.0 => return Err(at.error("División por cero.")),
                Divide => left / right,
                Mod if right.floor() == 0.0 => {
                    return Err(
                        at.error("El segundo operando para el operador MOD no puede ser cero.")
                    );
                }
                Mod if left.fract() == 0.0 && right.fract() == 0.0 => {
                    (left as i64).checked_rem(right as i64).unwrap_or(0) as f64
                }
                Mod => {
                    return Err(at.error("Los operandos para el operador MOD deben ser enteros."));
                }
                Power => left.powf(right),
                _ => return Err(at.error("Operador numérico inválido.")),
            };
            Ok(Number(result))
        }
        (Text(left), Text(right)) => Ok(match op {
            Plus => Text(left + &right),
            Eq => Logic(left == right),
            Ne => Logic(left != right),
            Lt => Logic(left < right),
            Le => Logic(left <= right),
            Gt => Logic(left > right),
            Ge => Logic(left >= right),
            _ => return Err(at.error("Operador inválido para cadenas de caracteres.")),
        }),
        (Logic(left), Logic(right)) => Ok(match op {
            Eq => Logic(left == right),
            Ne => Logic(left != right),
            _ => return Err(at.error("Operador inválido para valores lógicos.")),
        }),
        _ => Err(at.error("No coinciden los tipos.")),
    }
}
