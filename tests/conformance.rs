use std::io::{self, Cursor, Write};
use std::time::Duration;

use potoo_pseint_interpreter::{Host, run, run_with_host};

#[test]
fn arrays_keep_fractional_and_input_text_keys_while_checking_bounds_numerically() {
    let source = r#"
Algoritmo Indices
    Dimension valores[3]
    valores[1] <- 7
    valores[1.5] <- 9
    Leer indice
    valores[indice] <- 11
    Escribir valores[1], ":", valores[1.5], ":", valores[indice]
FinAlgoritmo
"#;
    let mut input = Cursor::new("01\n");
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "> 7:9:11\n");
}

#[test]
fn declarations_and_inference_supply_their_default_scalar_values() {
    let source = r#"
Algoritmo Tipos
    Definir entero Como Entero
    Definir real Como Real
    Definir logico Como Logico
    Definir texto Como Caracter
    Escribir entero, ":", real, ":", logico, ":", texto
    inferido <- 7
    frase <- "hola"
    condicion <- Verdadero
    Escribir inferido, ":", frase, ":", condicion
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "0:0:FALSO:\n7:hola:VERDADERO\n"
    );
}

#[test]
fn subprograms_preserve_value_copy_reference_and_array_passage() {
    let source = r#"
SubProceso IncrementarArreglo(valores)
    valores[1] <- valores[1] + 1
FinSubProceso

SubProceso Cambiar(valor Por Valor, copia Por Copia, referencia Por Referencia)
    valor <- valor + 1
    copia <- copia + 2
    referencia <- referencia + 3
FinSubProceso

Algoritmo Pasajes
    Dimension valores[1]
    valores[1] <- 1
    original <- 4
    copia <- 4
    referencia <- 4
    IncrementarArreglo(valores)
    Cambiar(original, copia, referencia)
    Escribir valores[1], ":", original, ":", copia, ":", referencia
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "2:4:4:7\n");
}

#[test]
fn recursive_functions_return_values_and_keep_each_invocation_local() {
    let source = r#"
Funcion resultado <- Factorial(numero)
    Si numero <= 1 Entonces
        resultado <- 1
    Sino
        resultado <- numero * Factorial(numero - 1)
    Fin Si
Fin Funcion

Algoritmo Recursion
    Escribir Factorial(5)
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "120\n");
}

#[test]
fn integer_constraints_remain_active_in_value_parameters() {
    let source = r#"
SubProceso Invalidar(valor Por Valor)
    valor <- 1.5
FinSubProceso

Algoritmo Entero
    Definir numero Como Entero
    numero <- 1
    Invalidar(numero)
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    let diagnostic = run(source, &mut input, &mut output).unwrap_err();

    assert!(diagnostic.message.contains("debe ser un entero"));
}

#[test]
fn all_control_structures_execute_their_documented_branch_and_iteration_rules() {
    let source = r#"
Algoritmo Control
    total <- 0
    contador <- 0
    Si 2 Es Mayor O Igual Que 2 Entonces
        Mientras contador < 2 Hacer
            total <- total + 1
            contador <- contador + 1
        Fin Mientras
    Sino
        total <- 99
    Fin Si
    Repetir
        total <- total + 1
    Hasta Que total = 3
    Repetir
        total <- total - 1
    Mientras Que total > 2
    Para contador <- 1 Hasta 2 Con Paso 1 Hacer
        total <- total + contador
    Fin Para
    Segun total Hacer
        4: marca <- 1
        5 O 6: marca <- 2
        De Otro Modo: marca <- 3
    Fin Segun
    Segun "azul" Hacer
        "azul": marca <- marca + 1
        De Otro Modo: marca <- 99
    Fin Segun
    Dimension valores[2]
    valores[1] <- 1
    valores[2] <- 2
    Para Cada elemento De valores Hacer
        elemento <- elemento + 1
    Fin Para
    Escribir total, ":", marca, ":", valores[1], ":", valores[2]
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "5:3:2:3\n");
}

#[test]
fn colloquial_comparisons_cover_parity_integrality_multiples_and_order() {
    let source = r#"
Algoritmo Comparaciones
    a <- 4
    b <- 6
    c <- 3.5
    Escribir a Es Par, ":", a No Es Par, ":", b Es Multiplo De a, ":", c Es Entero, ":", a Es Mayor Que b, ":", b Es Mayor O Igual Que a, ":", a Es Distinto De b
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "VERDADERO:FALSO:FALSO:FALSO:FALSO:VERDADERO:VERDADERO\n"
    );
}

#[test]
fn numeric_and_text_builtins_preserve_rounding_and_accented_positions() {
    let source = r#"
Algoritmo Funciones
    Escribir ABS(-2.5), ":", TRUNC(-2.9), ":", REDON(-2.5), ":", RAIZ(9), ":", Longitud("niño"), ":", SubCadena("mañana", 2, 4), ":", Mayúsculas("áñü"), ":", Minúsculas("ÁÑÜ"), ":", ConvertirATexto(1.25), ":", Concatenar("¡", "sí"), ":", ConvertirATexto(1.23456789127), ":", Longitud("a😀b"), ":", SubCadena("a😀b", 2, 2), ":", ConvertirANumero("-12.5")
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "2.5:-2:-3:3:4:aña:ÁÑÜ:áñü:1.25:¡sí:1.2345678913:3:😀:-12.5\n"
    );
}

#[test]
fn mathematical_builtins_use_radians_and_the_documented_constants() {
    let source = r#"
Algoritmo Matematicas
    Escribir RC(4), ":", LN(1), ":", EXP(0), ":", SEN(0), ":", ASEN(0), ":", COS(0), ":", ACOS(1), ":", TAN(0), ":", ATAN(0), ":", PI, ":", EULER
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run(source, &mut input, &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "2:0:1:0:0:1:0:0:0:3.1415926536:2.7182818285\n"
    );
}

#[test]
fn seeded_random_draws_match_the_linux_reference_sequence() {
    struct SeedOne;

    impl Host for SeedOne {
        fn random_seed(&mut self) -> u32 {
            1
        }
    }

    let source = "Algoritmo Sorteo\nEscribir Azar(100), \" \", Aleatorio(-10, 10), \" \", Azar(100)\nFinAlgoritmo";
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();

    run_with_host(source, &mut input, &mut output, &mut SeedOne).unwrap();

    assert_eq!(String::from_utf8(output).unwrap(), "83 -6 77\n");
}

#[test]
fn host_receives_wait_and_clear_operations_without_blocking_the_test() {
    #[derive(Default)]
    struct Recorder {
        clears: usize,
        key_waits: usize,
        waits: Vec<Duration>,
    }

    impl Host for Recorder {
        fn clear_screen(&mut self, output: &mut dyn Write) -> io::Result<()> {
            self.clears += 1;
            output.write_all(b"[clear]")
        }

        fn wait_for_key(&mut self, _input: &mut dyn io::BufRead) -> io::Result<()> {
            self.key_waits += 1;
            Ok(())
        }

        fn wait(&mut self, duration: Duration) -> io::Result<()> {
            self.waits.push(duration);
            Ok(())
        }
    }

    let source = r#"
Algoritmo Host
    Escribir Sin Saltar "A"
    Borrar Pantalla
    Limpiar Pantalla
    Esperar Tecla
    Esperar 2 Milisegundos
    Esperar 1 Segundo
    Escribir Sin Bajar "Z"
    Escribir "!"
FinAlgoritmo
"#;
    let mut input = Cursor::new(Vec::new());
    let mut output = Vec::new();
    let mut host = Recorder::default();

    run_with_host(source, &mut input, &mut output, &mut host).unwrap();

    assert_eq!(host.clears, 2);
    assert_eq!(host.key_waits, 1);
    assert_eq!(
        host.waits,
        [Duration::from_millis(2), Duration::from_secs(1)]
    );
    assert_eq!(String::from_utf8(output).unwrap(), "A[clear][clear]Z!\n");
}
