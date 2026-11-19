use crate::{Diagnostic, Position};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Word(String),
    Text(String),
    Number(f64),
    Assign,
    Plus,
    Minus,
    Star,
    Slash,
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
    DoubleOr,
    Not,
    Open,
    Close,
    Comma,
    Colon,
    Semicolon,
    Newline,
    Eof,
}

pub(crate) struct Token {
    pub kind: Kind,
    pub at: Position,
    pub spaced: bool,
}

fn is_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic() || "_áéíóúüñÁÉÍÓÚÜÑ".contains(ch)
}

pub(crate) fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    use Kind::*;
    let mut tokens = Vec::new();
    let mut at = Position { line: 1, column: 1 };
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    for (line, raw) in source.split('\n').enumerate() {
        let mut chars = raw.trim_end_matches('\r').chars().enumerate().peekable();
        let mut spaced = false;
        while let Some((column, ch)) = chars.next() {
            at = Position {
                line: line + 1,
                column: column + 1,
            };
            let mut eat = |next| chars.next_if(|(_, c)| *c == next).is_some();
            let kind = match ch {
                ' ' | '\t' => {
                    spaced = true;
                    continue;
                }
                '/' if eat('/') => break,
                ';' => Semicolon,
                ',' => Comma,
                '(' | '[' => Open,
                ')' | ']' => Close,
                '<' if eat('-') => Assign,
                ':' if eat('=') => Assign,
                ':' => Colon,
                '<' if eat('>') => Ne,
                '<' if eat('=') => Le,
                '>' if eat('=') => Ge,
                '!' if eat('=') => Ne,
                '=' => {
                    eat('=');
                    Eq
                }
                '<' => Lt,
                '>' => Gt,
                '+' => Plus,
                '-' => Minus,
                '*' => Star,
                '/' => Slash,
                '^' => Power,
                '%' => Mod,
                '&' => {
                    eat('&');
                    And
                }
                '|' if eat('|') => DoubleOr,
                '|' => Or,
                '~' | '!' => Not,
                '"' | '\'' => {
                    let mut value = String::new();
                    loop {
                        match chars.next() {
                            Some((_, '"' | '\'')) => break,
                            Some((_, next)) => value.push(next),
                            None => return Err(at.error("unterminated string")),
                        }
                    }
                    Text(value)
                }
                '0'..='9' | '.' => {
                    let mut value = String::from(ch);
                    let mut dot = ch == '.';
                    while let Some((_, next)) =
                        chars.next_if(|(_, c)| c.is_ascii_digit() || (*c == '.' && !dot))
                    {
                        dot |= next == '.';
                        value.push(next);
                    }
                    if let Some((_, exponent)) = chars.next_if(|(_, c)| matches!(c, 'e' | 'E')) {
                        value.push(exponent);
                        if let Some((_, sign @ ('+' | '-'))) =
                            chars.next_if(|(_, c)| matches!(c, '+' | '-'))
                        {
                            value.push(sign);
                        }
                        let mut exponent_digits = 0;
                        while let Some((_, digit)) = chars.next_if(|(_, c)| c.is_ascii_digit()) {
                            value.push(digit);
                            exponent_digits += 1;
                        }
                        if exponent_digits == 0 {
                            return Err(at.error("invalid number"));
                        }
                    }
                    Number(
                        value
                            .parse::<f64>()
                            .ok()
                            .ok_or_else(|| at.error("invalid number"))?,
                    )
                }
                c if is_letter(c) => {
                    let mut value = String::from(c);
                    while let Some((_, next)) =
                        chars.next_if(|(_, c)| is_letter(*c) || c.is_ascii_digit())
                    {
                        value.push(next);
                    }
                    let word = value.to_uppercase();
                    match word.as_str() {
                        "Y" => And,
                        "O" => Or,
                        "NO" => Not,
                        "MOD" => Mod,
                        _ => Word(word),
                    }
                }
                c => return Err(at.error(format!("unsupported character {c:?}"))),
            };
            tokens.push(Token { kind, at, spaced });
            spaced = false;
        }
        at = Position {
            line: line + 1,
            column: raw.trim_end_matches('\r').chars().count() + 1,
        };
        tokens.push(Token {
            kind: Newline,
            at,
            spaced: false,
        });
    }
    tokens.push(Token {
        kind: Eof,
        at,
        spaced: false,
    });
    Ok(tokens)
}
