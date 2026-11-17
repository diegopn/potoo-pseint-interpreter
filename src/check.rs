use std::collections::{HashMap, HashSet};

use crate::builtins;
use crate::parser::{
    Expr, Expression, Line, Name, Op, Passage, Place, Program, Routine, Statement, Type, Value,
};
use crate::{Diagnostic, Position};

pub(crate) type Types<'a> = HashMap<&'a str, Type>;

const NUMERIC: u8 = 1;
const TEXT: u8 = 2;
const LOGIC: u8 = 4;
const ALL: u8 = NUMERIC | TEXT | LOGIC;

type Variable = usize;
type TypeFlow = (Variable, Variable, Position);

struct TypeVariable {
    parent: Variable,
    possibilities: u8,
    rank: u32,
}

#[derive(Default)]
struct Constraints(Vec<TypeVariable>);

impl Constraints {
    fn new(&mut self, possibilities: u8) -> Variable {
        let id = self.0.len();
        self.0.push(TypeVariable {
            parent: id,
            possibilities,
            rank: 0,
        });
        id
    }

    fn root(&mut self, id: Variable) -> Variable {
        let parent = self.0[id].parent;
        if parent != id {
            self.0[id].parent = self.root(parent);
        }
        self.0[id].parent
    }

    fn require(&mut self, id: Variable, allowed: u8, at: Position) -> Result<(), Diagnostic> {
        let root = self.root(id);
        let possibilities = self.0[root].possibilities & allowed;
        if possibilities == 0 {
            return Err(at.error("No coinciden los tipos."));
        }
        self.0[root].possibilities = possibilities;
        Ok(())
    }

    fn unify(
        &mut self,
        first: Variable,
        second: Variable,
        at: Position,
    ) -> Result<Variable, Diagnostic> {
        let mut left = self.root(first);
        let mut right = self.root(second);
        if left != right {
            if self.0[left].rank < self.0[right].rank {
                std::mem::swap(&mut left, &mut right);
            }
            self.require(left, self.0[right].possibilities, at)?;
            self.0[right].parent = left;
            if self.0[left].rank == self.0[right].rank {
                self.0[left].rank += 1;
            }
        }
        Ok(left)
    }

    fn possibilities(&mut self, id: Variable) -> u8 {
        let root = self.root(id);
        self.0[root].possibilities
    }

    fn propagate(&mut self, flows: &[TypeFlow]) -> Result<(), Diagnostic> {
        loop {
            let mut changed = false;
            for &(source, target, at) in flows {
                let allowed = self.possibilities(source);
                let before = self.possibilities(target);
                if before & allowed != before {
                    self.require(target, allowed, at)?;
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }
}

#[derive(Default)]
struct Scope<'a> {
    variables: HashMap<&'a str, Variable>,
    explicit: HashMap<&'a str, Type>,
    ranks: HashMap<&'a str, usize>,
    parameters: HashSet<&'a str>,
}

struct Checker<'a, 'program> {
    constraints: Constraints,
    program: &'program Program<'a>,
    returns: HashMap<&'a str, Variable>,
    flows: Vec<TypeFlow>,
    arguments: Vec<((&'a str, &'a str), Variable, Position)>,
}

pub(crate) fn check<'a>(program: &Program<'a>) -> Result<Types<'a>, Diagnostic> {
    let mut checker = Checker {
        constraints: Constraints::default(),
        program,
        returns: HashMap::new(),
        flows: Vec::new(),
        arguments: Vec::new(),
    };
    let mut routine_names = HashSet::from([program.main.name.0]);
    if builtins::signature(program.main.name.0).is_some() {
        return Err(program
            .main
            .name
            .1
            .error("El nombre del proceso está reservado."));
    }
    for routine in &program.routines {
        let Name(name, at) = routine.name;
        if !routine_names.insert(name) || builtins::signature(name).is_some() {
            return Err(at.error(format!(
                "Ya existe otro proceso/subproceso con el mismo nombre ({name})."
            )));
        }
        if routine.result.is_some() {
            checker.returns.insert(name, checker.constraints.new(ALL));
        }
    }
    let mut parameters = HashMap::new();
    for routine in &program.routines {
        let scope = checker.scope(routine)?;
        for (param, _) in &routine.params {
            parameters.insert((routine.name.0, param.0), scope.variables[param.0]);
        }
    }
    let main = checker.scope(&program.main)?;
    checker.constraints.propagate(&checker.flows)?;
    // Parameter constraints validate calls without assigning a runtime type before the call.
    let main_types = main
        .variables
        .into_iter()
        .filter_map(|(name, variable)| {
            let ty = main.explicit.get(name).copied().or_else(|| {
                match checker.constraints.possibilities(variable) {
                    NUMERIC => Some(Type::Real),
                    TEXT => Some(Type::Text),
                    LOGIC => Some(Type::Logic),
                    _ => None,
                }
            });
            ty.map(|ty| (name, ty))
        })
        .collect();
    checker.flows.extend(
        checker
            .arguments
            .into_iter()
            .map(|(parameter, actual, at)| (parameters[&parameter], actual, at)),
    );
    checker.constraints.propagate(&checker.flows)?;
    Ok(main_types)
}

impl<'a> Checker<'a, '_> {
    fn scope(&mut self, routine: &Routine<'a>) -> Result<Scope<'a>, Diagnostic> {
        let mut scope = Scope::default();
        for &(name, _) in &routine.params {
            if !scope.parameters.insert(name.0) {
                return Err(name.1.error(format!("Argumento repetido ({}).", name.0)));
            }
            self.variable(&mut scope, name)?;
        }
        if let Some(name) = routine.result {
            let variable = self.variable(&mut scope, name)?;
            self.constraints
                .unify(variable, self.returns[routine.name.0], name.1)?;
        }
        self.block(&mut scope, &routine.body)?;
        Ok(scope)
    }

    fn variable(
        &mut self,
        scope: &mut Scope<'a>,
        Name(name, at): Name<'a>,
    ) -> Result<Variable, Diagnostic> {
        if name == self.program.main.name.0
            || self.routine(name).is_some()
            || builtins::signature(name).is_some()
        {
            return Err(at.error(format!("Identificador no válido ({name}).")));
        }
        Ok(*scope
            .variables
            .entry(name)
            .or_insert_with(|| self.constraints.new(ALL)))
    }

    fn routine(&self, name: &str) -> Option<&'a str> {
        self.program
            .routines
            .iter()
            .find(|routine| routine.name.0 == name)
            .map(|routine| routine.name.0)
    }

    fn place(
        &mut self,
        scope: &mut Scope<'a>,
        Place(name, indexes): &Place<'a>,
        whole_array: bool,
    ) -> Result<Variable, Diagnostic> {
        let variable = self.variable(scope, *name)?;
        match (scope.ranks.get(name.0), indexes) {
            (Some(rank), Some(indexes)) if *rank != indexes.len() => {
                return Err(name.1.error(format!(
                    "Cantidad de índices incorrecta para el arreglo ({}).",
                    name.0
                )));
            }
            (Some(_), None) if !whole_array => {
                return Err(name
                    .1
                    .error(format!("Faltan subíndices para el arreglo ({}).", name.0)));
            }
            (None, Some(_)) if !scope.parameters.contains(name.0) => {
                return Err(name.1.error(format!(
                    "El identificador {} no corresponde a un arreglo o subproceso.",
                    name.0
                )));
            }
            _ => {}
        }
        if let Some(indexes) = indexes {
            for expr in indexes {
                self.expect(scope, expr, NUMERIC)?;
            }
        }
        Ok(variable)
    }

    fn block(&mut self, scope: &mut Scope<'a>, lines: &[Line<'a>]) -> Result<(), Diagnostic> {
        for Line(statement, at) in lines {
            match statement {
                Statement::Define(names, ty) => {
                    for &name in names {
                        if scope.parameters.contains(name.0) {
                            return Err(name.1.error("No debe redefinir el tipo de un argumento."));
                        }
                        let variable = self.variable(scope, name)?;
                        self.constraints.require(variable, kind(*ty), name.1)?;
                        // Repeated declarations are checked when execution reaches them.
                        scope.explicit.entry(name.0).or_insert(*ty);
                    }
                }
                Statement::Dimension(arrays) => {
                    for Place(name, dimensions) in arrays {
                        if scope.parameters.contains(name.0) {
                            return Err(name.1.error("No debe redimensionar un argumento."));
                        }
                        self.variable(scope, *name)?;
                        let dimensions =
                            dimensions.as_ref().expect("dimensions required by parser");
                        for expr in dimensions {
                            self.expect(scope, expr, NUMERIC)?;
                        }
                        scope.ranks.insert(name.0, dimensions.len());
                    }
                }
                Statement::Assign(place, expr) => {
                    let target = self.place(scope, place, false)?;
                    let value = self.expression(scope, expr)?;
                    let expected = self.constraints.possibilities(target);
                    let actual = self.constraints.possibilities(value);
                    self.constraints.require(value, expected, expr.1)?;
                    self.constraints.require(target, actual, *at)?;
                }
                Statement::Read(places) => {
                    for place in places {
                        self.place(scope, place, false)?;
                    }
                }
                Statement::Write(expressions, _) => {
                    for expr in expressions {
                        self.expression(scope, expr)?;
                    }
                }
                Statement::Call(place) => {
                    self.call(scope, place, false)?;
                }
                Statement::If(condition, yes, no) => {
                    self.expect(scope, condition, LOGIC)?;
                    self.block(scope, yes)?;
                    self.block(scope, no)?;
                }
                Statement::While(condition, body) => {
                    self.expect(scope, condition, LOGIC)?;
                    self.block(scope, body)?;
                }
                Statement::Repeat(body, condition, _) => {
                    self.block(scope, body)?;
                    self.expect(scope, condition, LOGIC)?;
                }
                Statement::For(name, start, end, step, body) => {
                    let variable = self.place(scope, &Place(*name, None), false)?;
                    self.constraints.require(variable, NUMERIC, name.1)?;
                    self.expect(scope, start, NUMERIC)?;
                    self.expect(scope, end, NUMERIC)?;
                    if let Some(step) = step {
                        self.expect(scope, step, NUMERIC)?;
                    }
                    self.block(scope, body)?;
                }
                Statement::ForEach(name, array, body) => {
                    let element = self.place(scope, &Place(*name, None), false)?;
                    let array = self.place(scope, &Place(*array, None), true)?;
                    self.constraints.unify(element, array, *at)?;
                    self.block(scope, body)?;
                }
                Statement::Switch(selector, cases) => {
                    let selector_type = self.expression(scope, selector)?;
                    self.constraints
                        .require(selector_type, NUMERIC | TEXT, selector.1)?;
                    for (labels, body) in cases {
                        for label in labels {
                            let label_type = self.expression(scope, label)?;
                            self.constraints.unify(selector_type, label_type, label.1)?;
                        }
                        self.block(scope, body)?;
                    }
                }
                Statement::Wait(time, _) => self.expect(scope, time, NUMERIC)?,
                Statement::Clear | Statement::WaitKey => {}
            }
        }
        Ok(())
    }

    fn expect(
        &mut self,
        scope: &mut Scope<'a>,
        expr: &Expr<'a>,
        allowed: u8,
    ) -> Result<(), Diagnostic> {
        let variable = self.expression(scope, expr)?;
        self.constraints.require(variable, allowed, expr.1)
    }

    fn expression(
        &mut self,
        scope: &mut Scope<'a>,
        Expr(expr, at, _): &Expr<'a>,
    ) -> Result<Variable, Diagnostic> {
        use Op::*;
        let id = match expr {
            Expression::Literal(Value::Number(_)) => self.constraints.new(NUMERIC),
            Expression::Literal(Value::Logic(_)) => self.constraints.new(LOGIC),
            Expression::Literal(Value::Text(_)) => self.constraints.new(TEXT),
            Expression::Literal(Value::Empty | Value::Input(_)) => self.constraints.new(ALL),
            Expression::Variable(place) => {
                if self.routine(place.0.0).is_some() || builtins::signature(place.0.0).is_some() {
                    self.call(scope, place, true)?
                } else {
                    self.place(scope, place, false)?
                }
            }
            Expression::IsInteger(operand) => {
                self.expect(scope, operand, NUMERIC)?;
                self.constraints.new(LOGIC)
            }
            Expression::Unary(op, expr) => {
                let expected = if *op == Not { LOGIC } else { NUMERIC };
                self.expect(scope, expr, expected)?;
                self.constraints.new(expected)
            }
            Expression::Binary(op, left, right) => {
                let left = self.expression(scope, left)?;
                let right = self.expression(scope, right)?;
                let (allowed, result) = match op {
                    And | Or => (LOGIC, LOGIC),
                    Eq | Ne => (ALL, LOGIC),
                    Lt | Le | Gt | Ge => (NUMERIC | TEXT, LOGIC),
                    Plus => (NUMERIC | TEXT, 0),
                    Minus | Times | Divide | Power | Mod => (NUMERIC, NUMERIC),
                    Not => unreachable!("invalid binary operator"),
                };
                let equal = self.constraints.unify(left, right, *at)?;
                self.constraints.require(equal, allowed, *at)?;
                if result == 0 {
                    equal
                } else {
                    self.constraints.new(result)
                }
            }
        };
        Ok(id)
    }

    fn call(
        &mut self,
        scope: &mut Scope<'a>,
        Place(Name(name, at), expressions): &Place<'a>,
        value_required: bool,
    ) -> Result<Variable, Diagnostic> {
        let args = expressions.as_deref().unwrap_or(&[]);
        let bad_arity = || {
            at.error(format!(
                "Cantidad de argumentos incorrecta para el subproceso ({name})."
            ))
        };
        if let Some(signature) = builtins::signature(name) {
            if !value_required {
                return Err(at.error(format!(
                    "La función retorna un valor, debe ser parte de una expresión ({name})."
                )));
            }
            if args.len() != signature.params.len() {
                return Err(bad_arity());
            }
            for (arg, ty) in args.iter().zip(signature.params) {
                self.expect(scope, arg, kind(*ty))?;
            }
            return Ok(self.constraints.new(kind(signature.result)));
        }
        let routine = self
            .program
            .routines
            .iter()
            .find(|routine| routine.name.0 == *name)
            .ok_or_else(|| at.error(format!("Subproceso no definido ({name}).")))?;
        if args.len() != routine.params.len() {
            return Err(bad_arity());
        }
        match (value_required, routine.result.is_some()) {
            (true, false) => {
                return Err(at.error(format!("El subproceso ({name}) no devuelve ningún valor.")));
            }
            (false, true) => {
                return Err(at.error(format!(
                    "La función retorna un valor, debe ser parte de una expresión ({name})."
                )));
            }
            _ => {}
        }
        for (arg, (param, passage)) in args.iter().zip(&routine.params) {
            let actual = if let Expression::Variable(place) = &arg.0
                && self.routine(place.0.0).is_none()
                && builtins::signature(place.0.0).is_none()
            {
                let is_array = place.1.is_none() && scope.ranks.contains_key(place.0.0);
                if is_array && *passage == Passage::Value {
                    return Err(arg
                        .1
                        .error("Los arreglos solo pueden pasarse por referencia."));
                }
                self.place(scope, place, is_array)?
            } else {
                if *passage == Passage::Reference {
                    return Err(arg
                        .1
                        .error("No puede utilizar una expresión en un pasaje por referencia."));
                }
                self.expression(scope, arg)?
            };
            self.arguments
                .push(((routine.name.0, param.0), actual, arg.1));
        }
        if let Some(&result) = self.returns.get(name) {
            let declared = self.constraints.possibilities(result);
            let call_type = self.constraints.new(declared);
            self.flows.push((result, call_type, *at));
            Ok(call_type)
        } else {
            Ok(self.constraints.new(ALL))
        }
    }
}

fn kind(ty: Type) -> u8 {
    match ty {
        Type::Integer | Type::Real => NUMERIC,
        Type::Text => TEXT,
        Type::Logic => LOGIC,
    }
}
