#![forbid(unsafe_code)]

//! Interpreter for PSeInt’s default command-line profile.

mod builtins;
mod check;
mod lexer;
mod parser;
mod runtime;

use std::fmt;
use std::io::{self, BufRead, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Parser and evaluator recursion share this bound with the expression tree height.
const MAX_NESTING: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Position {
    line: usize,
    column: usize,
}

/// A source error with a one-based line and column.
#[derive(Debug)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub backtrace: Vec<CallSite>,
}

impl Position {
    fn error(self, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            line: self.line,
            column: self.column,
            message: message.into(),
            backtrace: Vec::new(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(writer, "{}:{}: {}", self.line, self.column, self.message)?;
        for frame in &self.backtrace {
            write!(
                writer,
                "\n...dentro del subproceso {}, invocado desde la línea {}.",
                frame.routine, frame.line
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostic {}

/// The caller location of a subprogram on a diagnostic's stack.
#[derive(Debug)]
pub struct CallSite {
    pub routine: String,
    pub line: usize,
    pub column: usize,
}

/// A line about to execute and the current invocation's local variables.
pub struct Step<'a> {
    pub line: usize,
    pub column: usize,
    pub routine: &'a str,
    pub call_depth: usize,
    pub(crate) inspect: &'a dyn Fn(&str, Option<&[i32]>) -> Option<Variable>,
}

impl Step<'_> {
    /// Reads a variable or an array element using the original one-based indices.
    pub fn variable(&self, name: &str, index: Option<&[i32]>) -> Option<Variable> {
        (self.inspect)(name, index)
    }
}

/// A runtime value as displayed by PSeInt, together with its current array bounds.
#[derive(Debug)]
pub struct Variable {
    pub value: Option<String>,
    pub dimensions: Vec<i32>,
}

/// Environment services used by a running program.
pub trait Host {
    /// Called after parsing and type checking, before execution starts.
    fn on_start(&mut self, _output: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }

    /// Called when the main process finishes successfully.
    fn on_finish(&mut self, _output: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }

    /// Clears the console and returns the cursor to its home position.
    fn clear_screen(&mut self, output: &mut dyn Write) -> io::Result<()> {
        output.write_all(b"\x1b[2J\x1b[1;1H")?;
        output.flush()
    }

    /// Reads one line for `Leer` after the interpreter writes and flushes its prompt.
    fn read_line(
        &mut self,
        input: &mut dyn BufRead,
        _output: &mut dyn Write,
        line: &mut String,
    ) -> io::Result<usize> {
        input.read_line(line)
    }

    /// Suspends execution until a key is available.
    fn wait_for_key(&mut self, input: &mut dyn BufRead) -> io::Result<()> {
        let mut key = [0];
        input.read_exact(&mut key)
    }

    /// Suspends execution for a numeric `Esperar` instruction.
    fn wait(&mut self, duration: Duration) -> io::Result<()> {
        std::thread::sleep(duration);
        Ok(())
    }

    /// Reports the next source location, allowing a debugger to pause or cancel execution.
    fn on_line(&mut self, _step: &Step<'_>) -> io::Result<()> {
        Ok(())
    }

    /// Provides the initial seed for `Azar` and `Aleatorio`.
    fn random_seed(&mut self) -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32
    }
}

struct Console;
impl Host for Console {}

/// Parses and checks a UTF-8 program, then executes it with console behavior.
pub fn run(
    source: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<(), Diagnostic> {
    run_with_host(source, input, output, &mut Console)
}

/// Runs a UTF-8 program with caller-supplied streams and environment services.
pub fn run_with_host(
    source: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
    host: &mut impl Host,
) -> Result<(), Diagnostic> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(&tokens)?;
    let types = check::check(&program)?;
    let at = program.main.name.1;
    host.on_start(output)
        .map_err(|err| at.error(err.to_string()))?;
    runtime::execute(&program, &types, input, output, host)?;
    host.on_finish(output)
        .map_err(|err| at.error(err.to_string()))
}

/// Checks syntax and inferred types without executing the program.
pub fn check(source: &str) -> Result<(), Diagnostic> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(&tokens)?;
    check::check(&program).map(|_| ())
}
