use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::rc::Rc;
use std::time::Duration;

use crate::builtins::{self, Random, decimal, display_number};
use crate::check::Types;
use crate::parser::{
    Expr, Expression, Line, Name, Op, Passage, Place, Program, Statement, Type, TypeSet, Value,
};
use crate::{CallSite, Diagnostic, Host, MAX_NESTING, Position, Step, Variable};

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
        return_types: &types.returns,
        input,
        output,
        host,
        random,
        line: String::new(),
        routine: program.main.name.0,
        call_depth: 0,
        nesting: 0,
    };
    runtime.block(&mut seed_frame(&types.main), &program.main.body)
}

type Frame<'a> = HashMap<&'a str, Address>;

struct Slot {
    types: TypeSet,
    integer: bool,
    declared: bool,
    scalar: Value,
    used: bool,
    array: Option<Array>,
}

impl Slot {
    fn ty(&self) -> Option<Type> {
        if self.integer {
            Some(Type::Integer)
        } else {
            self.types.known()
        }
    }

    fn constrain(&mut self, allowed: TypeSet, at: Position) -> Result<(), Diagnostic> {
        self.types = self
            .types
            .intersection(allowed)
            .ok_or_else(|| at.error("No coinciden los tipos."))?;
        Ok(())
    }
}

struct Array {
    dimensions: Vec<i32>,
    values: HashMap<Vec<String>, Value>,
}

#[derive(Clone)]
struct Address {
    slot: Rc<RefCell<Slot>>,
    index: Option<Vec<String>>,
}

impl Address {
    fn new(ty: Option<Type>, scalar: Value) -> Self {
        let slot = Rc::new(RefCell::new(Slot {
            types: ty.map_or_else(|| scalar.possible_types(), TypeSet::of),
            integer: ty == Some(Type::Integer),
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
        self.slot.borrow().ty()
    }

    fn get(&self, at: Position) -> Result<Value, Diagnostic> {
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
        Ok(match value {
            Value::Empty => slot.ty().map_or(Value::Empty, Type::default_value),
            Value::Input(text, _) => match slot.ty() {
                Some(Type::Real | Type::Integer) => Value::Number(value.as_number(at)?),
                Some(Type::Logic) => Value::Logic(value.as_bool(at)?),
                Some(Type::Text) => Value::Text(text.clone()),
                None => Value::Input(text.clone(), slot.types),
            },
            value => value.clone(),
        })
    }

    fn copy(&self, at: Position) -> Result<Self, Diagnostic> {
        let value = self.get(at)?;
        let slot = self.slot.borrow();
        let copy = Self::new(slot.ty(), value);
        copy.slot.borrow_mut().types = slot.types;
        Ok(copy)
    }

    fn set(&self, value: Value, at: Position) -> Result<(), Diagnostic> {
        self.store(value, at, true)
    }

    fn store(&self, value: Value, at: Position, check_integer: bool) -> Result<(), Diagnostic> {
        let mut slot = self.slot.borrow_mut();
        if check_integer && slot.integer && !Type::Integer.accepts(&value) {
            if Type::Real.accepts(&value) {
                return Err(
                    at.error("No coinciden los tipos, el valor a asignar debe ser un entero.")
                );
            }
            return Err(at.error("No coinciden los tipos."));
        }
        slot.constrain(value.possible_types(), at)?;
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
    return_types: &'run HashMap<&'a str, TypeSet>,
    input: &'run mut R,
    output: &'run mut W,
    host: &'run mut H,
    random: Random,
    line: String,
    routine: &'a str,
    call_depth: usize,
    nesting: usize,
}

impl<'a, R: BufRead, W: Write, H: Host> Runtime<'a, '_, R, W, H> {
    fn nested<T>(
        &mut self,
        at: Position,
        run: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        if self.nesting >= MAX_NESTING {
            return Err(at.error("Se superó el límite de anidamiento durante la ejecución."));
        }
        self.nesting += 1;
        let result = run(self);
        self.nesting -= 1;
        result
    }

    fn block(&mut self, frame: &mut Frame<'a>, statements: &[Line<'a>]) -> Result<(), Diagnostic> {
        let Some(first) = statements.first() else {
            return Ok(());
        };
        self.nested(first.1, |runtime| runtime.block_inner(frame, statements))
    }

    fn block_inner(
        &mut self,
        frame: &mut Frame<'a>,
        statements: &[Line<'a>],
    ) -> Result<(), Diagnostic> {
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
                        slot.types = TypeSet::of(*ty);
                        slot.integer = *ty == Type::Integer;
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
                    let start = self
                        .evaluate_as(frame, first, Type::Real)?
                        .as_number(first.1)?;
                    let stop = self
                        .evaluate_as(frame, last, Type::Real)?
                        .as_number(last.1)?;
                    let step = match step {
                        Some(expr) => self
                            .evaluate_as(frame, expr, Type::Real)?
                            .as_number(expr.1)?,
                        None if start > stop => -1.0,
                        None => 1.0,
                    };
                    let address = self.address(frame, &Place(*name, None), false)?;
                    address.store(Value::Number(start), name.1, false)?;
                    loop {
                        let counter = address.get(name.1)?.as_number(name.1)?;
                        if if step >= 0.0 {
                            counter > stop
                        } else {
                            counter < stop
                        } {
                            break;
                        }
                        self.block(frame, body)?;
                        self.on_line(frame, *at)?;
                        let counter = address.get(name.1)?.as_number(name.1)?;
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
                            index: Some(index.iter().map(i32::to_string).collect()),
                        };
                        element.set(entry.get(name.1)?, name.1)?;
                        self.block(frame, body)?;
                        entry.set(element.get(name.1)?, name.1)?;
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
                            if self
                                .evaluate_binary(frame, Op::Eq, selector, label, label.1)?
                                .as_bool(label.1)?
                            {
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
                if let Some(index) = &address.index {
                    (array.values.get(index), Vec::new())
                } else if let Some(index) = subscript {
                    if index.len() != array.dimensions.len()
                        || index
                            .iter()
                            .zip(&array.dimensions)
                            .any(|(index, end)| !(1..=*end).contains(index))
                    {
                        return None;
                    }
                    let key = index.iter().map(i32::to_string).collect::<Vec<_>>();
                    (array.values.get(&key), Vec::new())
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
            let value = self.evaluate(frame, expr)?;
            let truncated = value.as_number(expr.1)? as i32;
            let subscript = value.display();
            if truncated < 1 || truncated > upper {
                return Err(expr.1.error(format!(
                    "Subíndice ({subscript}) fuera de rango (1...{upper})."
                )));
            }
            // The reference checks the truncated index and uses the displayed value as its key.
            index.push(subscript);
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
            None => {
                let possible = match text {
                    "" => TypeSet::ANY,
                    "VERDADERO" | "FALSO" => TypeSet::TEXT_OR_LOGIC,
                    text if decimal(text).is_some() => TypeSet::TEXT_OR_NUMBER,
                    _ => TypeSet::TEXT,
                };
                Ok(Value::Input(text.to_owned(), possible))
            }
        }
    }

    fn condition(&mut self, frame: &mut Frame<'a>, expr: &Expr<'a>) -> Result<bool, Diagnostic> {
        self.evaluate_as(frame, expr, Type::Logic)?.as_bool(expr.1)
    }

    fn is_callable(&self, name: &str) -> bool {
        builtins::signature(name).is_some()
            || self
                .program
                .routines
                .iter()
                .any(|routine| routine.name.0 == name)
    }

    fn constrain_expression(
        &mut self,
        frame: &mut Frame<'a>,
        expr: &Expr<'a>,
        allowed: TypeSet,
    ) -> Result<(), Diagnostic> {
        self.nested(expr.1, |runtime| match &expr.0 {
            Expression::Variable(place) if !runtime.is_callable(place.0.0) => {
                Self::slot(frame, place.0)
                    .slot
                    .borrow_mut()
                    .constrain(allowed, expr.1)
            }
            Expression::Unary(Op::Plus | Op::Minus, operand) => {
                runtime.constrain_expression(frame, operand, allowed)
            }
            Expression::Binary(Op::Plus, left, right) => {
                let current = runtime.determine_sum_types(frame, left, right)?;
                if current == TypeSet::EMPTY || current.intersection(allowed) == Some(current) {
                    return Ok(());
                }
                runtime.constrain_expression(frame, left, allowed)?;
                runtime.constrain_expression(frame, right, allowed)
            }
            _ => Ok(()),
        })
    }

    fn apply_expected_type(
        &mut self,
        frame: &mut Frame<'a>,
        expr: &Expr<'a>,
        allowed: TypeSet,
    ) -> Result<(), Diagnostic> {
        self.nested(expr.1, |runtime| {
            if runtime
                .determine_types(frame, expr)?
                .intersection(allowed)
                .is_none()
            {
                return Err(expr.1.error("No coinciden los tipos."));
            }
            use Op::*;
            match &expr.0 {
                Expression::Unary(Plus | Minus, operand) => {
                    runtime.apply_type_if_needed(frame, operand, allowed)
                }
                Expression::Binary(
                    Plus | Minus | Times | Divide | Power | Mod | And | Or,
                    left,
                    right,
                ) => {
                    runtime.apply_type_if_needed(frame, left, allowed)?;
                    runtime.apply_type_if_needed(frame, right, allowed)
                }
                _ => runtime.constrain_expression(frame, expr, allowed),
            }
        })
    }

    fn apply_type_if_needed(
        &mut self,
        frame: &mut Frame<'a>,
        expr: &Expr<'a>,
        allowed: TypeSet,
    ) -> Result<(), Diagnostic> {
        if self.determine_types(frame, expr)? != allowed {
            self.apply_expected_type(frame, expr, allowed)?;
        }
        Ok(())
    }

    fn evaluate_as(
        &mut self,
        frame: &mut Frame<'a>,
        expr: &Expr<'a>,
        ty: Type,
    ) -> Result<Value, Diagnostic> {
        let allowed = TypeSet::of(ty);
        self.apply_expected_type(frame, expr, allowed)?;
        self.evaluate_operand(frame, expr, allowed)
    }

    fn evaluate(&mut self, frame: &mut Frame<'a>, expr: &Expr<'a>) -> Result<Value, Diagnostic> {
        self.nested(expr.1, |runtime| runtime.evaluate_inner(frame, expr))
    }

    fn evaluate_inner(
        &mut self,
        frame: &mut Frame<'a>,
        Expr(expr, at, _): &Expr<'a>,
    ) -> Result<Value, Diagnostic> {
        Ok(match expr {
            Expression::Literal(value) => value.clone(),
            Expression::Variable(place) => {
                if self.is_callable(place.0.0) {
                    self.call(frame, place)?
                } else {
                    self.address(frame, place, false)?.get(*at)?
                }
            }
            Expression::IsInteger(operand) => {
                let left = self
                    .evaluate_as(frame, operand, Type::Real)?
                    .as_number(*at)?;
                let right = self.evaluate(frame, operand)?.as_number(*at)?;
                Value::Logic((left - f64::from(builtins::trunc(right))).abs() < 1e-11)
            }
            Expression::Unary(op, operand) => {
                let allowed = if *op == Op::Not {
                    TypeSet::LOGIC
                } else {
                    TypeSet::NUMBER
                };
                let candidates = self.determine_types(frame, operand)?;
                if candidates == TypeSet::EMPTY {
                    self.evaluate(frame, operand)?;
                    return Err(at.error("No coinciden los tipos."));
                }
                if candidates != allowed {
                    self.apply_expected_type(frame, operand, allowed)?;
                }
                let value = self.evaluate_operand(frame, operand, allowed)?;
                match op {
                    Op::Not => Value::Logic(!value.as_bool(*at)?),
                    Op::Plus => Value::Number(value.as_number(*at)?),
                    Op::Minus => Value::Number(-value.as_number(*at)?),
                    _ => unreachable!("invalid unary operator"),
                }
            }
            Expression::Binary(op @ (Op::And | Op::Or), left, right) => {
                let (left_types, right_types) =
                    self.operator_operand_types(frame, left, right, *at)?;
                let allowed = TypeSet::LOGIC;
                if left_types != allowed {
                    self.apply_expected_type(frame, left, allowed)?;
                }
                if right_types != allowed {
                    self.apply_expected_type(frame, right, allowed)?;
                }
                let left_value = self
                    .evaluate_operand(frame, left, allowed)?
                    .as_bool(left.1)?;
                let value = if left_value == (*op == Op::Or) {
                    left_value
                } else {
                    self.evaluate_operand(frame, right, allowed)?
                        .as_bool(right.1)?
                };
                Value::Logic(value)
            }
            Expression::Binary(Op::Plus, left, right) => {
                self.evaluate_sum(frame, left, right, *at)?
            }
            Expression::Binary(op, left, right) => {
                self.evaluate_binary(frame, *op, left, right, *at)?
            }
        })
    }

    fn determine_types(
        &mut self,
        frame: &Frame<'a>,
        expr: &Expr<'a>,
    ) -> Result<TypeSet, Diagnostic> {
        self.nested(expr.1, |runtime| {
            use Op::*;
            Ok(match &expr.0 {
                Expression::Literal(value) => value.possible_types(),
                Expression::Variable(Place(Name(name, _), _)) => {
                    if let Some(signature) = builtins::signature(name) {
                        TypeSet::of(signature.result)
                    } else if let Some(&returns) = runtime.return_types.get(name) {
                        returns
                    } else {
                        frame
                            .get(name)
                            .map_or(TypeSet::ANY, |address| address.slot.borrow().types)
                    }
                }
                Expression::Unary(Not, _)
                | Expression::IsInteger(_)
                | Expression::Binary(And | Or | Eq | Ne | Lt | Le | Gt | Ge, _, _) => {
                    TypeSet::LOGIC
                }
                Expression::Binary(Plus, left, right) => {
                    runtime.determine_sum_types(frame, left, right)?
                }
                Expression::Unary(..) | Expression::Binary(..) => TypeSet::NUMBER,
            })
        })
    }

    fn determine_sum_types(
        &mut self,
        frame: &Frame<'a>,
        left: &Expr<'a>,
        right: &Expr<'a>,
    ) -> Result<TypeSet, Diagnostic> {
        let left = self.determine_types(frame, left)?;
        let right = self.determine_types(frame, right)?;
        Ok(addition_types(left, right).unwrap_or(TypeSet::EMPTY))
    }

    fn evaluate_operand(
        &mut self,
        frame: &mut Frame<'a>,
        expr: &Expr<'a>,
        allowed: TypeSet,
    ) -> Result<Value, Diagnostic> {
        let value = self.evaluate(frame, expr)?;
        if let Expression::Variable(Place(Name(name, _), _)) = &expr.0
            && self.is_callable(name)
            && value.possible_types().intersection(allowed).is_none()
        {
            return Err(expr.1.error("No coinciden los tipos."));
        }
        Ok(value)
    }

    fn operator_operand_types(
        &mut self,
        frame: &mut Frame<'a>,
        left: &Expr<'a>,
        right: &Expr<'a>,
        at: Position,
    ) -> Result<(TypeSet, TypeSet), Diagnostic> {
        let left_types = self.determine_types(frame, left)?;
        let right_types = self.determine_types(frame, right)?;
        if left_types == TypeSet::EMPTY || right_types == TypeSet::EMPTY {
            // Let the invalid operand report its error after the preceding operand has run.
            self.evaluate(frame, left)?;
            self.evaluate(frame, right)?;
            return Err(at.error("No coinciden los tipos."));
        }
        Ok((left_types, right_types))
    }

    fn evaluate_sum(
        &mut self,
        frame: &mut Frame<'a>,
        left: &Expr<'a>,
        right: &Expr<'a>,
        at: Position,
    ) -> Result<Value, Diagnostic> {
        let (left_types, right_types) = self.operator_operand_types(frame, left, right, at)?;
        let allowed =
            addition_types(left_types, right_types).ok_or_else(|| addition_type_error(at))?;
        if left_types != allowed {
            self.constrain_expression(frame, left, allowed)?;
        }
        if right_types != allowed {
            self.constrain_expression(frame, right, allowed)?;
        }

        let left = self.evaluate_operand(frame, left, allowed)?;
        let right = self.evaluate_operand(frame, right, allowed)?;
        if left
            .possible_types()
            .intersection(right.possible_types())
            .and_then(|possible| possible.intersection(allowed))
            .is_none()
        {
            return Err(addition_type_error(at));
        }
        // When both candidates remain, the reference adds numerically.
        if allowed == TypeSet::TEXT {
            Ok(Value::Text(
                [left.as_text(at)?, right.as_text(at)?].concat(),
            ))
        } else {
            Ok(Value::Number(
                addition_number(&left, at)? + addition_number(&right, at)?,
            ))
        }
    }

    fn evaluate_binary(
        &mut self,
        frame: &mut Frame<'a>,
        op: Op,
        left: &Expr<'a>,
        right: &Expr<'a>,
        at: Position,
    ) -> Result<Value, Diagnostic> {
        let (left_types, right_types) = self.operator_operand_types(frame, left, right, at)?;
        if matches!(op, Op::Minus | Op::Times | Op::Divide | Op::Power | Op::Mod) {
            let allowed = TypeSet::NUMBER;
            if left_types != allowed {
                self.apply_expected_type(frame, left, allowed)?;
            }
            if right_types != allowed {
                self.apply_expected_type(frame, right, allowed)?;
            }
            let left = self.evaluate_operand(frame, left, allowed)?;
            let right = self.evaluate_operand(frame, right, allowed)?;
            return binary(op, left, right, at);
        }
        let left_value = self.evaluate(frame, left)?;
        let right_value = self.evaluate(frame, right)?;
        if let Some(ty) = left_value.ty().or_else(|| right_value.ty()) {
            let allowed = TypeSet::of(ty);
            self.constrain_expression(frame, left, allowed)?;
            self.constrain_expression(frame, right, allowed)?;
        }
        binary(op, left_value, right_value, at)
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
                let expected = if *ty == Type::Integer {
                    Type::Real
                } else {
                    *ty
                };
                let value = self.evaluate_as(frame, arg, expected)?;
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
        // Explicit declarations and expression types take effect as this invocation executes.
        let mut locals = Frame::new();
        for (arg, (param, passage)) in args.iter().zip(&routine.params) {
            let place = match &arg.0 {
                Expression::Variable(place) if !self.is_callable(place.0.0) => Some(place),
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
            } else if let Some(place) = place {
                self.address(frame, place, false)?.copy(arg.1)?
            } else {
                let value = self.evaluate(frame, arg)?;
                Address::new(value.ty(), value)
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
        match routine.result {
            Some(name) => Self::slot(&mut locals, name).get(name.1),
            None => Ok(Value::Empty),
        }
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
        if !value.allows(self) {
            return false;
        }
        if self == Self::Integer && !matches!(value, Value::Empty) {
            let number = match value {
                Value::Number(n) => Some(*n),
                Value::Input(text, _) => decimal(text),
                _ => None,
            };
            return number.is_some_and(|n| {
                n.fract() == 0.0 && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&n)
            });
        }
        true
    }
}

impl Value {
    fn possible_types(&self) -> TypeSet {
        match self {
            Self::Number(_) => TypeSet::NUMBER,
            Self::Logic(_) => TypeSet::LOGIC,
            Self::Text(_) => TypeSet::TEXT,
            Self::Input(_, types) => *types,
            Self::Empty => TypeSet::ANY,
        }
    }

    fn allows(&self, ty: Type) -> bool {
        self.possible_types()
            .intersection(TypeSet::of(ty))
            .is_some()
    }

    fn ty(&self) -> Option<Type> {
        self.possible_types().known()
    }

    fn display(&self) -> String {
        match self {
            Self::Number(value) => display_number(*value),
            Self::Logic(true) => String::from("VERDADERO"),
            Self::Logic(false) => String::from("FALSO"),
            Self::Text(text) | Self::Input(text, _) => text.clone(),
            Self::Empty => String::new(),
        }
    }

    pub(crate) fn as_bool(&self, at: Position) -> Result<bool, Diagnostic> {
        match self {
            Self::Logic(value) => Ok(*value),
            Self::Input(text, _) if self.allows(Type::Logic) => Ok(text == "VERDADERO"),
            Self::Empty => Ok(false),
            _ => Err(at.error("Se esperaba un valor lógico.")),
        }
    }

    pub(crate) fn as_number(&self, at: Position) -> Result<f64, Diagnostic> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::Input(text, _) if self.allows(Type::Real) => {
                decimal(text).ok_or_else(|| at.error("Se esperaba un valor numérico."))
            }
            Self::Empty => Ok(0.0),
            _ => Err(at.error("Se esperaba un valor numérico.")),
        }
    }

    pub(crate) fn as_text(&self, at: Position) -> Result<&str, Diagnostic> {
        match self {
            Self::Text(value) => Ok(value),
            Self::Input(value, _) if self.allows(Type::Text) => Ok(value),
            Self::Empty => Ok(""),
            _ => Err(at.error("Se esperaba una cadena de caracteres.")),
        }
    }
}

fn addition_types(left: TypeSet, right: TypeSet) -> Option<TypeSet> {
    left.intersection(right)
        .and_then(|possible| possible.intersection(TypeSet::TEXT_OR_NUMBER))
}

fn addition_type_error(at: Position) -> Diagnostic {
    at.error("No coinciden los tipos (+). Los operandos deben ser de igual tipo.")
}

fn addition_number(value: &Value, at: Position) -> Result<f64, Diagnostic> {
    let text = match value {
        Value::Text(text) | Value::Input(text, _) => text,
        _ => return value.as_number(at),
    };
    // Numeric addition uses numeric string conversion after choosing a compatible operand type.
    let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
    if let Ok(number) = text.parse() {
        return Ok(number);
    }
    let mut end = 0;
    let mut dot = false;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b'+' | b'-' if index == 0 => {}
            b'0'..=b'9' => {}
            b'.' if !dot => dot = true,
            b'e' | b'E' => {
                let exponent = &text[index + 1..];
                let sign = usize::from(exponent.starts_with(['+', '-']));
                let digits = exponent[sign..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
                if digits > 0 {
                    end = index + 1 + sign + digits;
                }
                break;
            }
            _ => break,
        }
        end = index + 1;
    }
    Ok(text[..end].parse().unwrap_or(0.0))
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
        Empty | Input(..) => match ty {
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
