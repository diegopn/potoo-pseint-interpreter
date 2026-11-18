use std::io::{self, Cursor};

use potoo_pseint_interpreter::{Host, Step, check, run, run_with_host};

const COUNT_PROGRAM: &str = r#"
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
"#;

#[test]
fn check_accepts_the_cli_documentation_program_without_running_it() {
    check(COUNT_PROGRAM).unwrap();
}

#[test]
fn run_routes_input_and_executes_nested_conditions_and_loops() {
    let mut input = Cursor::new("2\n");
    let mut output = Vec::new();

    run(COUNT_PROGRAM, &mut input, &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Ingrese un límite:\n> 1 es impar\n2 es par\n"
    );
}

#[test]
fn host_observes_the_current_variable_before_the_statement_runs() {
    #[derive(Default)]
    struct Inspector {
        observed: Option<(usize, Option<String>)>,
    }

    impl Host for Inspector {
        fn on_line(&mut self, step: &Step<'_>) -> io::Result<()> {
            if step.line == 4 {
                self.observed = Some((
                    step.line,
                    step.variable("total", None).and_then(|v| v.value),
                ));
            }
            Ok(())
        }
    }

    let source =
        "Algoritmo Demo\nDefinir total Como Entero\ntotal <- 6\nEscribir total\nFinAlgoritmo";
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();
    let mut host = Inspector::default();

    run_with_host(source, &mut input, &mut output, &mut host).unwrap();

    assert_eq!(host.observed, Some((4, Some("6".to_owned()))));
    assert_eq!(String::from_utf8(output).unwrap(), "6\n");
}

#[test]
fn check_reports_source_coordinates_for_invalid_structure() {
    let diagnostic = check("Algoritmo Demo\nSi Verdadero Entonces\nFinAlgoritmo").unwrap_err();

    assert_eq!(diagnostic.line, 3);
    assert!(diagnostic.column > 0);
}
