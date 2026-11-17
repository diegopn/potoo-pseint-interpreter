#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use potoo_pseint_interpreter::{Host, check, run_with_host};

const USAGE: &str =
    "Usage: potoo-pseint [--nouser] [--norun] [--seed=N] [--input=VALUE] [--noinput] <file.psc>";

#[derive(Default)]
struct Terminal {
    quiet: bool,
    seed: Option<u32>,
    preset: VecDeque<String>,
    no_input: bool,
}

impl Host for Terminal {
    fn on_start(&mut self, output: &mut dyn Write) -> io::Result<()> {
        if !self.quiet {
            writeln!(output, "*** Ejecución Iniciada. ***")?;
        }
        Ok(())
    }

    fn on_finish(&mut self, output: &mut dyn Write) -> io::Result<()> {
        if !self.quiet {
            writeln!(output, "*** Ejecución Finalizada. ***")?;
        }
        Ok(())
    }

    fn random_seed(&mut self) -> u32 {
        self.seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as u32
        })
    }

    fn read_line(
        &mut self,
        input: &mut dyn BufRead,
        output: &mut dyn Write,
        line: &mut String,
    ) -> io::Result<usize> {
        if let Some(value) = self.preset.pop_front() {
            writeln!(output, "{value}")?;
            output.flush()?;
            line.push_str(&value);
            line.push('\n');
            Ok(line.len())
        } else if self.no_input {
            Ok(0)
        } else {
            input.read_line(line)
        }
    }

    fn wait_for_key(&mut self, input: &mut dyn BufRead) -> io::Result<()> {
        #[cfg(unix)]
        let mode = io::stdin()
            .is_terminal()
            .then(UnixKeyMode::open)
            .transpose()?;
        #[cfg(unix)]
        if mode.is_some() {
            // A terminal can encode a single key as multiple bytes.
            let count = input.fill_buf()?.len();
            if count == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            input.consume(count);
            return Ok(());
        }
        input.read_exact(&mut [0])
    }
}

#[cfg(unix)]
struct UnixKeyMode(String);

#[cfg(unix)]
impl UnixKeyMode {
    fn open() -> io::Result<Self> {
        use std::process::{Command, Stdio};
        let state = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()?;
        if !state.status.success() {
            return Err(io::Error::other("cannot read terminal settings"));
        }
        let mode = Self(String::from_utf8_lossy(&state.stdout).trim().to_owned());
        let status = Command::new("stty")
            .args(["-icanon", "-echo", "min", "1", "time", "0"])
            .status()?;
        if status.success() {
            Ok(mode)
        } else {
            Err(io::Error::other("cannot set terminal key mode"))
        }
    }
}

#[cfg(unix)]
impl Drop for UnixKeyMode {
    fn drop(&mut self) {
        let _ = std::process::Command::new("stty").arg(&self.0).status();
    }
}

fn main() -> ExitCode {
    let mut terminal = Terminal::default();
    let mut check_only = false;
    let mut path = None;
    for arg in env::args_os().skip(1) {
        match arg.to_str() {
            Some("--help" | "-h") => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            Some("--nouser") => terminal.quiet = true,
            Some("--norun") => check_only = true,
            Some("--noinput") => terminal.no_input = true,
            Some(option) if option.starts_with("--input=") => {
                terminal.preset.push_back(option[8..].to_owned())
            }
            Some(option) if option.starts_with("--seed=") => {
                let seed = option[7..].parse::<i32>();
                match seed {
                    Ok(-1) => terminal.seed = None,
                    Ok(seed) => terminal.seed = Some(seed as u32),
                    Err(_) => {
                        eprintln!("{USAGE}");
                        return ExitCode::from(2);
                    }
                }
            }
            Some(option) if option.starts_with("--") => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            _ if path.is_none() => path = Some(PathBuf::from(arg)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let source = match fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes)
            .unwrap_or_else(|err| err.into_bytes().into_iter().map(char::from).collect()),
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if !terminal.quiet {
        println!("Leyendo archivo y comprobando sintaxis...");
    }
    let result = if check_only {
        check(&source)
    } else {
        run_with_host(
            &source,
            &mut io::stdin().lock(),
            &mut io::stdout().lock(),
            &mut terminal,
        )
    };
    if let Err(err) = result {
        eprintln!("{}:{err}", path.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
