use std::f64::consts::{E, PI};

use crate::parser::{Type, Value};
use crate::{Diagnostic, Position};

use Type::{Integer, Real, Text};

pub(crate) struct Builtin {
    pub result: Type,
    pub params: &'static [Type],
}

pub(crate) fn signature(name: &str) -> Option<Builtin> {
    let (result, params): (_, &[_]) = match name {
        "PI" | "EULER" => (Real, &[]),
        "RC" | "RAIZ" | "ABS" | "LN" | "EXP" | "SEN" | "ASEN" | "COS" | "ACOS" | "TAN" | "ATAN"
        | "TRUNC" | "REDON" => (Real, &[Real]),
        "AZAR" => (Real, &[Integer]),
        "ALEATORIO" => (Real, &[Integer, Integer]),
        "LONGITUD" | "CONVERTIRANUMERO" | "CONVERTIRANÚMERO" => (Real, &[Text]),
        "CONVERTIRATEXTO" => (Text, &[Real]),
        "SUBCADENA" => (Text, &[Text, Integer, Integer]),
        "MAYUSCULAS" | "MAYÚSCULAS" | "MINUSCULAS" | "MINÚSCULAS" => (Text, &[Text]),
        "CONCATENAR" => (Text, &[Text, Text]),
        _ => return None,
    };
    Some(Builtin { result, params })
}

pub(crate) fn evaluate(
    name: &str,
    args: &[Value],
    at: Position,
    random: &mut Random,
) -> Result<Value, Diagnostic> {
    let number = |i: usize| args[i].as_number(at);
    let text = |i: usize| args[i].as_text(at);
    let value = match name {
        "PI" => PI,
        "EULER" => E,
        "RC" | "RAIZ" => {
            let x = number(0)?;
            if x < 0.0 {
                return Err(at.error("Raíz cuadrada de número negativo."));
            }
            x.sqrt()
        }
        "ABS" => number(0)?.abs(),
        "LN" => {
            let x = number(0)?;
            if x <= 0.0 {
                return Err(at.error("Logaritmo de 0 o negativo."));
            }
            x.ln()
        }
        "EXP" => number(0)?.exp(),
        "SEN" => number(0)?.sin(),
        "COS" => number(0)?.cos(),
        "TAN" => number(0)?.tan(),
        "ATAN" => number(0)?.atan(),
        "ASEN" | "ACOS" => {
            let x = number(0)?;
            if !(-1.0..=1.0).contains(&x) {
                return Err(at.error(format!(
                    "Argumento inválido para {name} (debe estar en [-1;+1])."
                )));
            }
            if name == "ASEN" { x.asin() } else { x.acos() }
        }
        "TRUNC" => f64::from(trunc(number(0)?)),
        "REDON" => f64::from(number(0)?.round() as i32),
        "AZAR" => {
            let bound = number(0)? as i32;
            if bound <= 0 {
                return Err(at.error("Azar de 0 o negativo."));
            }
            f64::from(random.next() % bound as u32)
        }
        "ALEATORIO" => {
            let a = i64::from(number(0)? as i32);
            let b = i64::from(number(1)? as i32);
            (a.min(b) + i64::from(random.next()) % ((b - a).abs() + 1)) as f64
        }
        "LONGITUD" => text(0)?.chars().count() as f64,
        "CONVERTIRANUMERO" | "CONVERTIRANÚMERO" => {
            let text = text(0)?;
            decimal(text).ok_or_else(|| {
                at.error(format!("La cadena (\"{text}\") no representa un número."))
            })?
        }
        "CONVERTIRATEXTO" => return Ok(Value::Text(display_number(number(0)?))),
        "CONCATENAR" => return Ok(Value::Text([text(0)?, text(1)?].concat())),
        "SUBCADENA" => {
            let text = text(0)?;
            let first = (number(1)? as i32).saturating_sub(1).max(0);
            let last = number(2)? as i32;
            let length = last.saturating_sub(first).max(0);
            return Ok(Value::Text(
                text.chars()
                    .skip(first as usize)
                    .take(length as usize)
                    .collect(),
            ));
        }
        "MAYUSCULAS" | "MAYÚSCULAS" | "MINUSCULAS" | "MINÚSCULAS" => {
            let upper = name.starts_with("MA");
            return Ok(Value::Text(
                text(0)?.chars().map(|ch| spanish_case(ch, upper)).collect(),
            ));
        }
        _ => unreachable!("builtin must be in signature table"),
    };
    Ok(Value::Number(value))
}

fn spanish_case(ch: char, upper: bool) -> char {
    const LOWER: &str = "áéíóúüñ";
    const UPPER: &str = "ÁÉÍÓÚÜÑ";
    let (from, to) = if upper {
        (LOWER, UPPER)
    } else {
        (UPPER, LOWER)
    };
    from.chars()
        .position(|c| c == ch)
        .and_then(|i| to.chars().nth(i))
        .unwrap_or_else(|| {
            if upper {
                ch.to_ascii_uppercase()
            } else {
                ch.to_ascii_lowercase()
            }
        })
}

pub(crate) fn display_number(value: f64) -> String {
    let decimal = format!("{value:.10}");
    let decimal = decimal.trim_end_matches('0').trim_end_matches('.');
    match decimal {
        "-0" => String::from("0"),
        "NaN" if value.is_sign_negative() => String::from("-nan"),
        "NaN" => String::from("nan"),
        _ => decimal.to_owned(),
    }
}

pub(crate) fn decimal(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut cursor = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let mut mantissa_digits = 0;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
        mantissa_digits += 1;
    }
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
            mantissa_digits += 1;
        }
    }
    if mantissa_digits == 0 {
        return None;
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let exponent_start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if cursor == exponent_start {
            return None;
        }
    }
    (cursor == bytes.len()).then(|| text.parse().ok()).flatten()
}

pub(crate) fn trunc(number: f64) -> i32 {
    let whole = number as i32;
    if f64::from(whole) + 1.0 - number < 1e-11 {
        whole.saturating_add(1)
    } else if f64::from(whole) - 1.0 - number > -1e-11 {
        whole.saturating_sub(1)
    } else {
        whole
    }
}

// The default Linux interpreter uses the libc additive generator.
pub(crate) struct Random {
    state: [u32; 31],
    cursor: usize,
}

impl Random {
    pub fn new(seed: u32) -> Self {
        let mut random = Self {
            state: [0; 31],
            cursor: 3,
        };
        let mut value = i64::from(seed.max(1) as i32);
        random.state[0] = value as u32;
        for entry in &mut random.state[1..] {
            value = (16_807 * value).rem_euclid(2_147_483_647);
            *entry = value as u32;
        }
        for _ in 0..310 {
            random.next();
        }
        random
    }

    fn next(&mut self) -> u32 {
        let i = self.cursor;
        let j = (i + 28) % 31;
        self.state[i] = self.state[i].wrapping_add(self.state[j]);
        self.cursor = (i + 1) % 31;
        self.state[i] >> 1
    }
}
