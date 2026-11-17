use crate::lexer::{Kind, Token};
use crate::{Diagnostic, Position};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Type {
    Integer,
    Real,
    Logic,
    Text,
}

#[derive(Clone)]
pub(crate) enum Value {
    Number(f64),
    Logic(bool),
    Text(String),
    Empty,
    Input(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    Plus,
    Minus,
    Times,
    Divide,
    Power,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
}

#[derive(Clone, Copy)]
pub(crate) struct Name<'a>(pub &'a str, pub Position);

#[derive(Clone)]
pub(crate) struct Expr<'a>(pub Expression<'a>, pub Position);

#[derive(Clone)]
pub(crate) enum Expression<'a> {
    Literal(Value),
    Variable(Place<'a>),
    IsInteger(Box<Expr<'a>>),
    Unary(Op, Box<Expr<'a>>),
    Binary(Op, Box<Expr<'a>>, Box<Expr<'a>>),
}

#[derive(Clone)]
pub(crate) struct Place<'a>(pub Name<'a>, pub Option<Vec<Expr<'a>>>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Passage {
    Default,
    Value,
    Reference,
}

pub(crate) struct Routine<'a> {
    pub name: Name<'a>,
    pub result: Option<Name<'a>>,
    pub params: Vec<(Name<'a>, Passage)>,
    pub body: Vec<Line<'a>>,
}

pub(crate) struct Program<'a> {
    pub main: Routine<'a>,
    pub routines: Vec<Routine<'a>>,
}

pub(crate) struct Line<'a>(pub Statement<'a>, pub Position);

type Case<'a> = (Vec<Expr<'a>>, Vec<Line<'a>>);

pub(crate) enum Statement<'a> {
    Define(Vec<Name<'a>>, Type),
    Dimension(Vec<Place<'a>>),
    Assign(Place<'a>, Expr<'a>),
    Read(Vec<Place<'a>>),
    Write(Vec<Expr<'a>>, bool),
    Call(Place<'a>),
    If(Expr<'a>, Vec<Line<'a>>, Vec<Line<'a>>),
    While(Expr<'a>, Vec<Line<'a>>),
    Repeat(Vec<Line<'a>>, Expr<'a>, bool),
    For(
        Name<'a>,
        Expr<'a>,
        Expr<'a>,
        Option<Expr<'a>>,
        Vec<Line<'a>>,
    ),
    ForEach(Name<'a>, Name<'a>, Vec<Line<'a>>),
    Switch(Expr<'a>, Vec<Case<'a>>),
    Clear,
    WaitKey,
    Wait(Expr<'a>, u64),
}

const MAIN: &[&str] = &["PROCESO", "ALGORITMO"];
const SUBPROGRAM: &[&str] = &["SUBPROCESO", "SUBALGORITMO", "FUNCION", "FUNCIÓN"];

pub(crate) fn parse(tokens: &[Token]) -> Result<Program<'_>, Diagnostic> {
    let mut p = Parser { tokens, index: 0 };
    let mut main = None;
    let mut routines = Vec::new();
    p.skip_ends();
    while p.current().kind != Kind::Eof {
        if p.any_word(MAIN) {
            if main.is_some() {
                return Err(p.error("only one Algoritmo or Proceso is allowed"));
            }
            let name = p.name()?;
            p.newline()?;
            let body = p.block(MAIN, Stop::End)?;
            p.closing(MAIN)?;
            p.newline()?;
            main = Some(Routine {
                name,
                result: None,
                params: Vec::new(),
                body,
            });
        } else if p.any_word(SUBPROGRAM) {
            routines.push(p.routine()?);
        } else {
            return Err(p.error("expected Algoritmo, Proceso, or a subprogram"));
        }
    }
    let main = main.ok_or_else(|| p.error("expected Algoritmo or Proceso"))?;
    Ok(Program { main, routines })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    End,
    Else,
    Repeat,
    Case,
}

struct Parser<'a> {
    tokens: &'a [Token],
    index: usize,
}

impl<'a> Parser<'a> {
    fn current(&self) -> &'a Token {
        &self.tokens[self.index]
    }

    fn error(&self, message: &str) -> Diagnostic {
        self.current().at.error(message)
    }

    fn take(&mut self, expected: Kind) -> bool {
        if self.current().kind == expected {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn word_at(&self, index: usize, expected: &str) -> bool {
        matches!(self.tokens.get(index).map(|t| &t.kind), Some(Kind::Word(word)) if word == expected)
    }

    fn word(&mut self, expected: &str) -> bool {
        if self.word_at(self.index, expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn any_word(&mut self, words: &[&str]) -> bool {
        words.iter().any(|word| self.word(word))
    }

    fn expect(&mut self, kind: Kind, message: &str) -> Result<(), Diagnostic> {
        if self.take(kind) {
            Ok(())
        } else {
            Err(self.error(message))
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<(), Diagnostic> {
        if self.word(word) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {word}")))
        }
    }

    fn routine(&mut self) -> Result<Routine<'a>, Diagnostic> {
        let first = self.name()?;
        let result = (self.take(Kind::Assign) || self.take(Kind::Eq)).then_some(first);
        let name = if result.is_some() {
            self.name()?
        } else {
            first
        };
        let mut params = Vec::new();
        if self.take(Kind::Open) && !self.take(Kind::Close) {
            loop {
                let param = self.name()?;
                let passage = if self.word("POR") {
                    if self.any_word(&["VALOR", "COPIA"]) {
                        Passage::Value
                    } else {
                        self.expect_word("REFERENCIA")?;
                        Passage::Reference
                    }
                } else {
                    Passage::Default
                };
                params.push((param, passage));
                if !self.take(Kind::Comma) {
                    break;
                }
            }
            self.expect(Kind::Close, "expected ')' after parameters")?;
        }
        self.newline()?;
        let body = self.block(SUBPROGRAM, Stop::End)?;
        self.closing(SUBPROGRAM)?;
        self.newline()?;
        Ok(Routine {
            name,
            result,
            params,
            body,
        })
    }

    fn skip_ends(&mut self) {
        while self.take(Kind::Newline) || self.take(Kind::Semicolon) {}
    }

    fn end(&mut self) -> Result<(), Diagnostic> {
        if !matches!(self.current().kind, Kind::Newline | Kind::Semicolon) {
            return Err(self.error("expected a line break or ';'"));
        }
        self.skip_ends();
        Ok(())
    }

    fn newline(&mut self) -> Result<(), Diagnostic> {
        if !self.take(Kind::Newline) {
            return Err(self.error("expected a line break"));
        }
        while self.take(Kind::Newline) {}
        Ok(())
    }

    fn is_closing(&self, ending: &str) -> bool {
        matches!(&self.current().kind, Kind::Word(s) if s.strip_prefix("FIN") == Some(ending))
            || (self.word_at(self.index, "FIN") && self.word_at(self.index + 1, ending))
    }

    fn closing(&mut self, endings: &[&str]) -> Result<(), Diagnostic> {
        let Some(ending) = endings.iter().find(|end| self.is_closing(end)) else {
            return Err(self.error(&format!("expected Fin{}", endings[0].to_ascii_lowercase())));
        };
        self.index += if self.word_at(self.index, "FIN") && self.word_at(self.index + 1, ending) {
            2
        } else {
            1
        };
        Ok(())
    }

    fn name(&mut self) -> Result<Name<'a>, Diagnostic> {
        let token = self.current();
        let Kind::Word(name) = &token.kind else {
            return Err(self.error("expected an identifier"));
        };
        if matches!(
            name.as_str(),
            "ALGORITMO"
                | "PROCESO"
                | "FIN"
                | "FINPROCESO"
                | "FINALGORITMO"
                | "DEFINIR"
                | "COMO"
                | "LEER"
                | "ESCRIBIR"
                | "IMPRIMIR"
                | "MOSTRAR"
                | "INFORMAR"
                | "SI"
                | "ENTONCES"
                | "SINO"
                | "FINSI"
                | "MIENTRAS"
                | "HACER"
                | "FINMIENTRAS"
                | "VERDADERO"
                | "FALSO"
                | "DIMENSION"
                | "DIMENSIÓN"
                | "SUBPROCESO"
                | "SUBALGORITMO"
                | "FINSUBALGORITMO"
                | "FINSUBPROCESO"
                | "FUNCION"
                | "FUNCIÓN"
                | "FINFUNCION"
                | "FINFUNCIÓN"
                | "PARA"
                | "PARACADA"
                | "FINPARA"
                | "REPETIR"
                | "HASTA"
                | "SEGUN"
                | "SEGÚN"
                | "FINSEGUN"
                | "FINSEGÚN"
                | "ESPERAR"
                | "ESPERARTECLA"
                | "LIMPIAR"
                | "BORRAR"
                | "PANTALLA"
                | "LIMPIARPANTALLA"
                | "BORRARPANTALLA"
                | "SIN"
                | "BAJAR"
                | "SINBAJAR"
                | "SALTAR"
                | "SINSALTAR"
        ) {
            return Err(token.at.error(format!("unexpected keyword {name}")));
        }
        self.index += 1;
        Ok(Name(name, token.at))
    }

    fn sequence_at(&self, words: &[&str]) -> bool {
        words
            .iter()
            .enumerate()
            .all(|(i, word)| self.word_at(self.index + i, word))
    }

    fn block(&mut self, endings: &[&str], stop: Stop) -> Result<Vec<Line<'a>>, Diagnostic> {
        let mut body = Vec::new();
        while !endings.iter().any(|end| self.is_closing(end))
            && !match stop {
                Stop::Else => self.word_at(self.index, "SINO"),
                Stop::Repeat => {
                    self.sequence_at(&["HASTA", "QUE"]) || self.sequence_at(&["MIENTRAS", "QUE"])
                }
                Stop::Case => self.case_start(),
                Stop::End => false,
            }
            && self.current().kind != Kind::Eof
        {
            body.push(self.statement()?);
        }
        Ok(body)
    }

    fn case_start(&self) -> bool {
        if self.sequence_at(&["DE", "OTRO", "MODO"]) {
            return true;
        }
        let mut depth = 0;
        for token in &self.tokens[self.index..] {
            match token.kind {
                Kind::Newline | Kind::Semicolon | Kind::Eof => break,
                Kind::Open => depth += 1,
                Kind::Close => depth -= 1,
                Kind::Colon if depth == 0 => return true,
                Kind::Assign if depth == 0 => break,
                _ => {}
            }
        }
        false
    }

    fn type_name(&mut self) -> Result<Type, Diagnostic> {
        let Kind::Word(word) = &self.current().kind else {
            return Err(self.error("expected a scalar type"));
        };
        let ty = match word.as_str() {
            "ENTERO" | "ENTEROS" | "ENTERA" | "ENTERAS" => Type::Integer,
            "REAL" | "REALES" | "NUMERO" | "NÚMERO" | "NUMEROS" | "NÚMEROS" | "NUMERICO"
            | "NUMÉRICO" | "NUMERICOS" | "NUMÉRICOS" | "NUMERICA" | "NUMÉRICA" | "NUMERICAS"
            | "NUMÉRICAS" => Type::Real,
            "LOGICO" | "LÓGICO" | "LOGICOS" | "LÓGICOS" | "LOGICA" | "LÓGICA" | "LOGICAS"
            | "LÓGICAS" => Type::Logic,
            "CARACTER" | "CARÁCTER" | "CARACTERES" | "CADENA" | "CADENAS" | "TEXTO" | "TEXTOS" => {
                Type::Text
            }
            _ => return Err(self.error("expected a scalar type")),
        };
        self.index += 1;
        Ok(ty)
    }

    fn names(&mut self, first: Name<'a>) -> Result<Vec<Name<'a>>, Diagnostic> {
        let mut names = vec![first];
        while self.take(Kind::Comma) {
            names.push(self.name()?);
        }
        Ok(names)
    }

    fn for_header(&mut self, each: bool) -> Result<Statement<'a>, Diagnostic> {
        let name = self.name()?;
        if each {
            if !self.any_word(&["DE", "EN"]) {
                return Err(self.error("expected De or En"));
            }
            let array = self.name()?;
            self.word("HACER");
            self.newline()?;
            let body = self.block(&["PARA"], Stop::End)?;
            self.closing(&["PARA"])?;
            return Ok(Statement::ForEach(name, array, body));
        }
        if !self.word("DESDE") {
            self.expect(Kind::Assign, "expected '<-' or Desde")?;
        }
        let first = self.expr()?;
        let early_step = if self.word("CON") {
            self.expect_word("PASO")?;
            Some(self.expr()?)
        } else {
            None
        };
        self.expect_word("HASTA")?;
        let last = self.expr()?;
        self.word("HACER");
        let step = if self.word("CON") {
            self.expect_word("PASO")?;
            let step = self.expr()?;
            self.word("HACER");
            if early_step.is_some() {
                return Err(self.error("duplicate Con Paso"));
            }
            Some(step)
        } else {
            early_step
        };
        self.newline()?;
        let body = self.block(&["PARA"], Stop::End)?;
        self.closing(&["PARA"])?;
        Ok(Statement::For(name, first, last, step, body))
    }

    fn switch(&mut self) -> Result<Statement<'a>, Diagnostic> {
        let selector = self.expr()?;
        self.word("HACER");
        self.newline()?;
        let mut cases = Vec::new();
        while !self.is_closing("SEGUN") && !self.is_closing("SEGÚN") {
            let mut labels = Vec::new();
            if self.sequence_at(&["DE", "OTRO", "MODO"]) {
                self.index += 3;
                self.take(Kind::Colon);
            } else {
                if self.sequence_at(&["SI", "ES"]) {
                    self.index += 2;
                } else {
                    self.any_word(&["CASO", "SIES", "OPCION", "OPCIÓN"]);
                }
                labels.push(self.expr()?);
                while self.take(Kind::Comma) {
                    labels.push(self.expr()?);
                }
                self.expect(Kind::Colon, "expected ':' after case label")?;
            }
            self.skip_ends();
            let body = self.block(&["SEGUN", "SEGÚN"], Stop::Case)?;
            cases.push((labels, body));
        }
        self.closing(&["SEGUN", "SEGÚN"])?;
        Ok(Statement::Switch(selector, cases))
    }

    fn statement(&mut self) -> Result<Line<'a>, Diagnostic> {
        let at = self.current().at;
        let statement = if self.word("DEFINIR") {
            let first = self.name()?;
            let names = self.names(first)?;
            self.expect_word("COMO")?;
            Statement::Define(names, self.type_name()?)
        } else if self.any_word(&["DIMENSION", "DIMENSIÓN"]) {
            let mut arrays = Vec::new();
            loop {
                let name = self.name()?;
                let place = self.place(name)?;
                if place.1.as_ref().is_none_or(Vec::is_empty) {
                    return Err(name.1.error("expected array dimensions"));
                }
                arrays.push(place);
                if !self.take(Kind::Comma) {
                    break;
                }
            }
            Statement::Dimension(arrays)
        } else if self.word("LEER") {
            let mut targets = Vec::new();
            loop {
                let name = self.name()?;
                targets.push(self.place(name)?);
                if !(self.take(Kind::Comma)
                    || (self.current().spaced && matches!(self.current().kind, Kind::Word(_))))
                {
                    break;
                }
            }
            Statement::Read(targets)
        } else if self.any_word(&["ESCRIBIR", "IMPRIMIR", "MOSTRAR", "INFORMAR"]) {
            let mut newline = !self.no_newline();
            let mut values = vec![self.expr()?];
            loop {
                if self.no_newline() {
                    newline = false;
                }
                if !(self.take(Kind::Comma) || (self.current().spaced && self.starts_expr())) {
                    break;
                }
                if self.no_newline() {
                    newline = false;
                }
                values.push(self.expr()?);
            }
            Statement::Write(values, newline)
        } else if self.word("SI") {
            let condition = self.expr()?;
            if self.current().kind == Kind::Newline {
                self.newline()?;
            }
            self.word("ENTONCES");
            self.skip_ends();
            let yes = self.block(&["SI"], Stop::Else)?;
            let no = if self.word("SINO") {
                self.skip_ends();
                self.block(&["SI"], Stop::End)?
            } else {
                Vec::new()
            };
            self.closing(&["SI"])?;
            Statement::If(condition, yes, no)
        } else if self.word("MIENTRAS") {
            let condition = self.expr()?;
            self.word("HACER");
            self.newline()?;
            let body = self.block(&["MIENTRAS"], Stop::End)?;
            self.closing(&["MIENTRAS"])?;
            Statement::While(condition, body)
        } else if self.any_word(&["REPETIR", "HACER"]) {
            self.end()?;
            let body = self.block(&[], Stop::Repeat)?;
            let while_true = self.word("MIENTRAS");
            if !while_true {
                self.expect_word("HASTA")?;
            }
            self.expect_word("QUE")?;
            Statement::Repeat(body, self.expr()?, while_true)
        } else if self.word("PARACADA") {
            self.for_header(true)?
        } else if self.word("PARA") {
            let each = self.word("CADA");
            self.for_header(each)?
        } else if self.any_word(&["SEGUN", "SEGÚN"]) {
            self.switch()?
        } else if self.any_word(&["BORRARPANTALLA", "LIMPIARPANTALLA"]) {
            Statement::Clear
        } else if self.any_word(&["BORRAR", "LIMPIAR"]) {
            self.expect_word("PANTALLA")?;
            Statement::Clear
        } else if self.word("ESPERARTECLA") {
            Statement::WaitKey
        } else if self.word("ESPERAR") {
            if self.word("UNA") {
                self.expect_word("TECLA")?;
                Statement::WaitKey
            } else if self.word("TECLA") {
                Statement::WaitKey
            } else {
                let span = self.expr()?;
                let factor = if self.any_word(&["SEGUNDO", "SEGUNDOS"]) {
                    1000
                } else {
                    if !self.any_word(&["MILISEGUNDO", "MILISEGUNDOS"]) {
                        return Err(self.error("expected Segundos or Milisegundos"));
                    }
                    1
                };
                Statement::Wait(span, factor)
            }
        } else {
            let name = self.name()?;
            if self.current().kind == Kind::Comma
                || self.sequence_at(&["ES"])
                || self.sequence_at(&["SON"])
            {
                let names = self.names(name)?;
                if !self.any_word(&["ES", "SON"]) {
                    return Err(self.error("expected Es or Son"));
                }
                Statement::Define(names, self.type_name()?)
            } else {
                let place = self.place(name)?;
                if self.take(Kind::Assign) {
                    Statement::Assign(place, self.expr()?)
                } else {
                    Statement::Call(place)
                }
            }
        };
        self.end()?;
        Ok(Line(statement, at))
    }

    fn place(&mut self, name: Name<'a>) -> Result<Place<'a>, Diagnostic> {
        if !self.take(Kind::Open) {
            return Ok(Place(name, None));
        }
        let mut arguments = Vec::new();
        if !self.take(Kind::Close) {
            arguments.push(self.infix(0)?);
            while self.take(Kind::Comma) {
                arguments.push(self.infix(0)?);
            }
            self.expect(Kind::Close, "expected closing parenthesis or bracket")?;
        }
        Ok(Place(name, Some(arguments)))
    }

    fn no_newline(&mut self) -> bool {
        if self.word("SINSALTAR") || self.word("SINBAJAR") {
            return true;
        }
        if self.word_at(self.index, "SIN")
            && ["SALTAR", "BAJAR"]
                .iter()
                .any(|w| self.word_at(self.index + 1, w))
        {
            self.index += 2;
            return true;
        }
        false
    }

    fn starts_expr(&self) -> bool {
        matches!(
            self.current().kind,
            Kind::Number(_) | Kind::Text(_) | Kind::Open | Kind::Word(_) | Kind::Not
        )
    }

    fn expr(&mut self) -> Result<Expr<'a>, Diagnostic> {
        self.infix(0)
    }

    fn infix(&mut self, min: u8) -> Result<Expr<'a>, Diagnostic> {
        let at = self.current().at;
        let expr = match &self.current().kind {
            Kind::Number(n) => {
                self.index += 1;
                Expression::Literal(Value::Number(*n))
            }
            Kind::Text(s) => {
                self.index += 1;
                Expression::Literal(Value::Text(s.clone()))
            }
            Kind::Word(w) if w == "VERDADERO" || w == "FALSO" => {
                self.index += 1;
                Expression::Literal(Value::Logic(w == "VERDADERO"))
            }
            Kind::Plus | Kind::Minus | Kind::Not => {
                let (op, binding) = match self.current().kind {
                    Kind::Plus => (Op::Plus, 7),
                    Kind::Minus => (Op::Minus, 7),
                    _ => (Op::Not, 3),
                };
                self.index += 1;
                Expression::Unary(op, Box::new(self.infix(binding)?))
            }
            Kind::Open => {
                self.index += 1;
                let inner = self.infix(0)?;
                if !self.take(Kind::Close) {
                    return Err(self.error("expected ')'"));
                }
                inner.0
            }
            Kind::Word(_) => {
                let name = self.name()?;
                Expression::Variable(self.place(name)?)
            }
            _ => return Err(self.error("expected an expression")),
        };
        let mut left = Expr(expr, at);
        loop {
            if min <= 3
                && (self.sequence_at(&["ES"])
                    || (self.current().kind == Kind::Not && self.word_at(self.index + 1, "ES")))
            {
                left = self.colloquial(left)?;
                continue;
            }
            let Some((op, precedence)) = self.operator() else {
                break;
            };
            if precedence < min {
                break;
            }
            let at = self.current().at;
            self.index += 1;
            let right = self.infix(precedence + 1)?;
            left = Expr(Expression::Binary(op, Box::new(left), Box::new(right)), at);
        }
        Ok(left)
    }

    fn colloquial(&mut self, left: Expr<'a>) -> Result<Expr<'a>, Diagnostic> {
        let at = self.current().at;
        let negate = self.take(Kind::Not);
        self.expect_word("ES")?;
        let number = |value| Expr(Expression::Literal(Value::Number(value)), at);
        let binary =
            |op, left, right| Expr(Expression::Binary(op, Box::new(left), Box::new(right)), at);
        let condition = if self.any_word(&["ENTERO", "ENTERA"]) {
            Expr(Expression::IsInteger(Box::new(left)), at)
        } else if self.word("PAR") {
            binary(Op::Eq, binary(Op::Mod, left, number(2.0)), number(0.0))
        } else if self.word("IMPAR") {
            binary(Op::Eq, binary(Op::Mod, left, number(2.0)), number(1.0))
        } else if self.any_word(&["POSITIVO", "POSITIVA"]) {
            binary(Op::Gt, left, number(0.0))
        } else if self.any_word(&["NEGATIVO", "NEGATIVA"]) {
            binary(Op::Lt, left, number(0.0))
        } else if self.word("CERO") {
            binary(Op::Eq, left, number(0.0))
        } else if self.any_word(&["DIVISIBLE", "MULTIPLO"]) {
            if !self.any_word(&["POR", "DE"]) {
                return Err(self.error("expected Por or De"));
            }
            let right = self.infix(3)?;
            binary(Op::Eq, binary(Op::Mod, left, right), number(0.0))
        } else {
            let op = if self.any_word(&["DISTINTO", "DISTINTA"]) {
                if !self.any_word(&["A", "DE"]) {
                    return Err(self.error("expected A or De"));
                }
                Op::Ne
            } else if self.any_word(&["IGUAL", "MAYOR", "MENOR"]) {
                let first = &self.tokens[self.index - 1].kind;
                let or_equal = self.take(Kind::Or);
                let other = if or_equal && self.any_word(&["IGUAL", "MAYOR", "MENOR"]) {
                    Some(&self.tokens[self.index - 1].kind)
                } else {
                    None
                };
                if !self.any_word(&["A", "QUE"]) {
                    return Err(self.error("expected A or Que"));
                }
                let gt = Kind::Word("MAYOR".into());
                let lt = Kind::Word("MENOR".into());
                if first == &gt || other == Some(&gt) {
                    if or_equal { Op::Ge } else { Op::Gt }
                } else if first == &lt || other == Some(&lt) {
                    if or_equal { Op::Le } else { Op::Lt }
                } else {
                    Op::Eq
                }
            } else {
                Op::Eq
            };
            let right = self.infix(3)?;
            binary(op, left, right)
        };
        Ok(if negate {
            Expr(Expression::Unary(Op::Not, Box::new(condition)), at)
        } else {
            condition
        })
    }

    fn operator(&self) -> Option<(Op, u8)> {
        Some(match self.current().kind {
            Kind::Or => (Op::Or, 1),
            Kind::And => (Op::And, 2),
            Kind::Eq => (Op::Eq, 3),
            Kind::Ne => (Op::Ne, 4),
            Kind::Lt => (Op::Lt, 4),
            Kind::Le => (Op::Le, 4),
            Kind::Gt => (Op::Gt, 4),
            Kind::Ge => (Op::Ge, 4),
            Kind::Plus => (Op::Plus, 5),
            Kind::Minus => (Op::Minus, 5),
            Kind::Star => (Op::Times, 6),
            Kind::Slash => (Op::Divide, 6),
            Kind::Mod => (Op::Mod, 6),
            Kind::Power => (Op::Power, 7),
            _ => return None,
        })
    }
}
