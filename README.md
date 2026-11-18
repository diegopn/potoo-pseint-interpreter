<p align="center">
  <img src="https://diegopn.github.io/potoo-site/src/assets/images/language-interpreter-icon.svg?v=1c651ca7" alt="Potoo IDE language interpreter icon" width="120" />
</p>

<h1 align="center">PSeInt Interpreter</h1>

<p align="center">An independent Rust interpreter library and CLI for the PSeInt pseudolanguage.</p>

<p align="center">
  <a href="https://github.com/diegopn/potoo-pseint-interpreter/releases/latest">Download</a> ·
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/pseint/">User manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

Originally developed as part of the Potoo IDE project, this interpreter is an independent Rust software package intended to be distributed separately from the IDE. It can be embedded as a library in other Rust projects or run as a standalone command-line tool on Linux, macOS, and Windows.

PSeInt is a pseudolanguage for expressing algorithms in Spanish. This interpreter targets the original version 20220506 and its `<predeterminado>` profile.

## Original language references

- [PSeInt official website](https://pseint.sourceforge.net/)
- [PSeInt pseudocode reference](https://pseint.sourceforge.net/index.php?page=pseudocodigo.php)
- [PSeInt features and language profiles](https://pseint.sourceforge.net/index.php?page=features.php)
- [Default profile settings](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/LangSettings.cpp)
- [Bundled language help for version 20220506](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/bin/help/)
- [Reference interpreter source](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/)

The original source is distributed under [GPL v2](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/license.txt); this independent implementation has its own license below.

## Source example

Save a UTF-8 file as `cuenta.psc`:

```text
Algoritmo Cuenta
    Definir limite, contador Como Enteros
    Escribir "Ingrese un límite:"
    Leer limite
    contador <- 1
    Mientras contador <= limite Hacer
        Si contador MOD 2 = 0 Entonces
            Escribir contador, " es par"
        Sino
            Escribir contador, " es impar"
        Fin Si
        contador <- contador + 1
    Fin Mientras
FinAlgoritmo
```

## Standalone command-line runner

Install [Rust and Cargo](https://www.rust-lang.org/tools/install) first. This project requires Rust 1.93 or newer.

```sh
git clone --branch develop https://github.com/diegopn/potoo-pseint-interpreter.git
cd potoo-pseint-interpreter
cargo install --path . --locked --bin potoo-pseint
```

Cargo places the executable in its configured binary directory, usually `~/.cargo/bin`. Add that directory to your `PATH` if the `potoo-pseint` command is not found.

To build without installing:

```sh
cargo build --release --locked --bin potoo-pseint
```

The executable is written to `target/release/` (`potoo-pseint.exe` on Windows).

Run the sample file from the repository root with:

```sh
cargo run --locked --bin potoo-pseint -- cuenta.psc
```

### Command-line usage

The runner accepts one `.psc` source file and these options:

```text
potoo-pseint [--nouser] [--norun] [--seed=N] [--input=VALUE] [--noinput] <file.psc>
```

- `--nouser` suppresses the CLI's checking, start, and finish messages, leaving program output visible.
- `--norun` checks syntax and types without executing the program.
- `--input=VALUE` supplies one value to the next `Leer`. Repeat the option for additional reads.
- `--noinput` makes execution use only the supplied values instead of waiting on standard input.
- `--seed=N` supplies a repeatable seed for `Azar` and `Aleatorio`. `--seed=-1` uses a time-based seed.
- `--help` prints the usage line. Invalid options and argument counts return status 2; source and runtime errors return a failure status with a file and source location.

Examples:

```sh
potoo-pseint cuenta.psc
potoo-pseint --norun cuenta.psc
potoo-pseint --nouser --input=4 --noinput cuenta.psc
potoo-pseint --nouser --seed=42 cuenta.psc
```

The CLI reads UTF-8 and falls back to ISO-8859-1 when needed. Program input and output use the terminal.

## Use as a Rust library

Use `potoo_pseint_interpreter::check` for source validation or `run` to execute a UTF-8 program with caller-supplied input and output streams. `run_with_host` accepts a `Host` for reading input, clearing the console, waiting, seeding random draws, and receiving execution events. The host can pause or cancel at a source location and inspect local variables or array elements through `Step`. Source diagnostics include one-based line and column positions and preserve the caller locations of runtime errors in subprograms.

Add a local checkout as a path dependency:

```toml
[dependencies]
potoo-pseint-interpreter = { path = "../potoo-pseint-interpreter" }
```

This example checks and runs a program with caller-owned input and output streams:

```rust
use std::io::{self, Cursor};
use potoo_pseint_interpreter::{check, run};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
Algoritmo Suma
    Definir a, b Como Enteros
    Leer a, b
    Escribir a + b
FinAlgoritmo
"#;

    check(source)?;
    let mut input = Cursor::new("2\n3\n");
    let mut output = Vec::new();
    run(source, &mut input, &mut output)?;
    print!("{}", String::from_utf8(output)?);
    Ok(())
}
```

For IDE integration, implement `Host` and call `run_with_host`. The host receives `on_line` callbacks before statements execute; `Step::variable` reads current scalar or array values. Override input, console clearing, waits, random seeding, or execution callbacks to match the embedding environment. The built-in `run` entry point uses the default console host.

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
git diff --check
```

## Support

If this project is useful to you, you can [sponsor its maintainer](https://github.com/sponsors/diegopn).

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
