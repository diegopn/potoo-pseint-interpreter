<h1 align="center">PSeInt Interpreter</h1>

<p align="center">A Rust library and CLI for running PSeInt programs.</p>

<p align="center">
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/pseint/">User manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

PSeInt is a pseudolanguage for expressing algorithms in Spanish. This independent implementation targets the original interpreter's `<predeterminado>` profile: the settings used by the command-line interpreter when no profile is supplied. The reference is the [original source at 5ee1dc2](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/), where [the interpreter version](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/version.h) is `20220506`. The original source is distributed under [GPL v2](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/license.txt); this implementation has its own license below.

## Run a program

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

From the repository root, run:

```sh
cargo run --locked --bin potoo-pseint -- cuenta.psc
```

The CLI displays PSeInt's execution messages. Pass `--nouser` to obtain only program output or `--norun` to check the program. Use `--input=VALUE` more than once to provide successive values to `Leer`, and add `--noinput` to use only those values. `--seed=N` sets the seed for repeatable random draws.

## Install the command-line runner

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

## Use the command-line runner

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

The CLI reads UTF-8 and falls back to ISO-8859-1 when the file is not valid UTF-8. It uses the terminal for program input and output. Interactive key waiting on Unix uses `stty` when standard input is a terminal; stream input consumes the next byte instead.

## Language and runtime

- `Algoritmo` and `Proceso` define the entry point. Identifiers are case insensitive and can use Spanish accented letters, digits after the first character, and underscores. Source supports `//` comments, optional semicolons for sequential statements, and spaced closing keywords such as `Fin Si`.
- `Definir … Como …` and `… Es/Son …` declare `Entero`, `Real`, `Lógico`, and `Carácter`, including their aliases. The interpreter checks the program before execution, infers types in the main process, and binds subprogram locals as each invocation runs. Numeric, logical, and character variables with a known type default to `0`, `FALSO`, and the empty string. When an input value has more than one possible type, later reads and typed operations retain the types compatible with all values seen so far.
- `Dimension` declares homogeneous arrays of one or more dimensions. Each dimension can be an expression. Arrays use one-based bounds and may be passed to subprograms by reference. Subprogram parameters cannot be dimensioned again. The reference interpreter truncates fractional indices for bounds checks and uses their displayed numeric forms as storage keys, so `A[1]` and `A[1.5]` can address different elements. An index supplied by untyped input can also retain leading zeros in its key.
- Assignment uses `<-` or `:=`. Expressions support arithmetic, power, remainder, text concatenation, comparisons, and short-circuit logic. The profile also accepts colloquial comparisons such as `x No Es Par`, `x Es Multiplo De 5`, and `x Es Mayor O Igual Que 3`.
- Control flow includes `Si … Entonces … Sino`, `Según` with numeric or text labels separated by commas, `O`, or `|`, `De Otro Modo`, `Mientras`, `Repetir … Hasta Que`, `Repetir … Mientras Que`, `Para` with an explicit or inferred step, and `Para Cada` with element updates. In `Mientras`, `Para`, and `Para Cada`, the first body statement may follow `Hacer` on the same line. A `Según` case label may also follow `Hacer` on the same line.
- `Función`, `SubProceso`, and `SubAlgoritmo` declare subprograms with local variables and optional return values. Calls support recursion, `Por Valor`/`Por Copia`, and `Por Referencia`. A variable declared as `Entero` keeps its integer constraint when passed by value.
- Built-ins include `PI`, `EULER`, `RC`/`Raiz`, `ABS`, `LN`, `EXP`, `SEN`, `ASEN`, `COS`, `ACOS`, `TAN`, `ATAN`, `TRUNC`, `REDON`, `AZAR`, `ALEATORIO`, `Longitud`, `SubCadena`, `Concatenar`, `Mayúsculas`, `Minúsculas`, `ConvertirANúmero`, and `ConvertirATexto`.
- `Leer` writes and flushes `> ` before taking one input line per destination. `Escribir` accepts several expressions and supports `Sin Saltar` and `Sin Bajar`. The numeric formatter rounds to ten decimal places and removes trailing zeroes. [Original display formatter](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/DataValue.h)
- `Borrar Pantalla` and `Limpiar Pantalla` clear the console. `Esperar Tecla` requests a key, and `Esperar` can take seconds or milliseconds.

## Library integration

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

The parser and runtime cap nested blocks, expressions, and calls at 64 active levels. The parser also caps expression tree height at 64, including chains of binary operators. Each level uses the Rust thread stack, where an overflow can abort the host process. A positioned diagnostic reports a program that exceeds the cap.

The CLI accepts UTF-8 and falls back to ISO-8859-1. The library uses Unicode text, including Unicode character positions for `Longitud` and `SubCadena`. These positions coincide with the original single-byte indices for ISO-8859-1 text. Diagnostics use the CLI's `file:line:column: message` format and a nonzero exit status. After a program finishes, this CLI returns to the caller immediately. A program can request a key explicitly with `Esperar Tecla`. The default seeded random sequence follows the Linux libc implementation. Other PSeInt language profiles and the original IDE interface are outside this interpreter's target.

## Original language references

- [PSeInt official website](https://pseint.sourceforge.net/)
- [PSeInt pseudocode reference](https://pseint.sourceforge.net/index.php?page=pseudocodigo.php)
- [PSeInt features and language profiles](https://pseint.sourceforge.net/index.php?page=features.php)
- [PSeInt profile examples](https://pseint.sourceforge.net/perfiles.php)
- [Default profile settings](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/LangSettings.cpp)
- [Bundled language help for version 20220506](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/bin/help/)
- [Reference interpreter source](https://sourceforge.net/p/pseint/code/ci/5ee1dc260181191e1cfa83fe02726f6812f6481c/tree/pseint/)

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
git diff --check
```

The integration tests in `tests/` exercise validation, source diagnostics, execution with input/output streams, host inspection callbacks, and the command-line flags.

## Support

If this project is useful to you, you can [sponsor its maintainer](https://github.com/sponsors/diegopn).

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
