//! Tests de la gramática.
//!
//! `ZIRK_SPEC_FINAL.md` sección 8 exige un caso válido y uno inválido por regla.
//! Los tests se agrupan por regla para que la correspondencia sea verificable.

use zirk_ast::*;
use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile};
use zirk_lexer::tokenize;
use zirk_parser::{codes, parse};

/// Parsea esperando que no haya errores.
fn programa(fuente: &str) -> Program {
    let source = SourceFile::new("test.zrk", fuente);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let programa = parse(&source, &tokens, &mut sink);

    assert!(
        !sink.has_errors(),
        "no se esperaban errores:\n{}",
        sink.render(RenderStyle::Human)
    );
    programa
}

/// Parsea esperando error, y devuelve los diagnósticos renderizados.
fn errores(fuente: &str) -> String {
    let source = SourceFile::new("test.zrk", fuente);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    parse(&source, &tokens, &mut sink);

    assert!(sink.has_errors(), "se esperaba un error y no hubo ninguno");
    sink.render(RenderStyle::Human)
}

/// Parsea el cuerpo de `main`, para tests centrados en sentencias.
fn sentencias(cuerpo: &str) -> Vec<Stmt> {
    let p = programa(&format!("fn main(): Void {{ {cuerpo} }}"));
    p.functions.into_iter().next().unwrap().body.statements
}

/// Parsea una expresión y la devuelve.
fn expresion(expr: &str) -> Expr {
    match sentencias(&format!("mut x = {expr};")).remove(0) {
        Stmt::Let(l) => l.init.expect("la declaración debe tener inicializador"),
        otro => panic!("se esperaba una declaración, se obtuvo {otro:?}"),
    }
}

/// Representación textual de una expresión, con paréntesis explícitos.
///
/// Hace verificable la forma del árbol sin depender de su estructura interna:
/// un test de precedencia dice `(1 + (2 * 3))` y se lee de inmediato.
fn forma(e: &Expr) -> String {
    match e {
        Expr::Int(i) => i.value.to_string(),
        Expr::Str(s) => format!("{:?}", s.value),
        Expr::Bool(b) => b.value.to_string(),
        Expr::Path(i) => i.name.clone(),
        Expr::Unary(u) => format!("({}{})", u.op.as_str(), forma(&u.operand)),
        Expr::Binary(b) => format!("({} {} {})", forma(&b.left), b.op.as_str(), forma(&b.right)),
        Expr::Call(c) => {
            let args: Vec<_> = c.args.iter().map(forma).collect();
            format!("{}({})", c.callee.name, args.join(", "))
        }
        Expr::Println(p) => format!("println({})", forma(&p.arg)),
    }
}

// --- Declaración de funciones -----------------------------------------------

#[test]
fn valido_funcion_sin_parametros() {
    let p = programa("fn main(): Void { }");
    let f = &p.functions[0];

    assert_eq!(f.name.name, "main");
    assert!(f.params.is_empty());
    assert_eq!(f.return_type.name, "Void");
}

#[test]
fn valido_funcion_con_parametros() {
    let p = programa("fn add(a: Int32, b: Int32): Int32 { return a + b; }");
    let f = &p.functions[0];

    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name.name, "a");
    assert_eq!(f.params[0].ty.name, "Int32");
    assert_eq!(f.return_type.name, "Int32");
}

#[test]
fn valido_varias_funciones_en_un_archivo() {
    let p = programa("fn a(): Void { } fn b(): Void { }");
    assert_eq!(p.functions.len(), 2);
}

#[test]
fn invalido_funcion_sin_tipo_de_retorno() {
    let salida = errores("fn main() { }");
    assert!(salida.contains(codes::FALTA_TIPO_RETORNO.as_str()));
    assert!(salida.contains("= ayuda:"));
}

#[test]
fn invalido_funcion_sin_nombre() {
    let salida = errores("fn (): Void { }");
    assert!(salida.contains(codes::TOKEN_INESPERADO.as_str()));
}

// --- Declaración de variables -----------------------------------------------

#[test]
fn valido_variable_con_tipo_explicito() {
    let Stmt::Let(l) = sentencias("mut count: Int32 = 0;").remove(0) else {
        panic!("se esperaba una declaración");
    };

    assert_eq!(l.mutability, Mutability::Mutable);
    assert_eq!(l.name.name, "count");
    assert_eq!(l.ty.unwrap().name, "Int32");
    assert!(l.init.is_some());
}

#[test]
fn valido_variable_con_tipo_inferido() {
    let Stmt::Let(l) = sentencias("mut count = 0;").remove(0) else {
        panic!("se esperaba una declaración");
    };

    assert!(l.ty.is_none());
    assert!(l.init.is_some());
}

#[test]
fn valido_variable_inmutable() {
    let Stmt::Let(l) = sentencias("inmut NAME: String = \"Zirk\";").remove(0) else {
        panic!("se esperaba una declaración");
    };

    assert_eq!(l.mutability, Mutability::Immutable);
}

#[test]
fn invalido_declaracion_sin_tipo_ni_inicializador() {
    let salida = errores("fn main(): Void { mut count; }");
    assert!(salida.contains(codes::DECLARACION_SIN_TIPO.as_str()));
    assert!(salida.contains("= ayuda:"));
}

#[test]
fn invalido_inmut_strict_no_esta_implementado() {
    let salida = errores("fn main(): Void { inmut::strict X: Int32 = 1; }");
    assert!(salida.contains(codes::NO_IMPLEMENTADO.as_str()));
}

// --- Precedencia y asociatividad --------------------------------------------

#[test]
fn valido_multiplicativo_tiene_mas_precedencia_que_aditivo() {
    assert_eq!(forma(&expresion("1 + 2 * 3")), "(1 + (2 * 3))");
    assert_eq!(forma(&expresion("1 * 2 + 3")), "((1 * 2) + 3)");
}

#[test]
fn valido_los_operadores_asocian_a_izquierda() {
    assert_eq!(forma(&expresion("10 - 4 - 3")), "((10 - 4) - 3)");
    assert_eq!(forma(&expresion("8 / 4 / 2")), "((8 / 4) / 2)");
}

#[test]
fn valido_los_parentesis_alteran_la_precedencia() {
    assert_eq!(forma(&expresion("(1 + 2) * 3")), "((1 + 2) * 3)");
}

#[test]
fn valido_la_conjuncion_tiene_mas_precedencia_que_la_disyuncion() {
    assert_eq!(forma(&expresion("a || b && c")), "(a || (b && c))");
    assert_eq!(forma(&expresion("a && b || c")), "((a && b) || c)");
}

#[test]
fn valido_la_comparacion_tiene_mas_precedencia_que_la_igualdad() {
    assert_eq!(forma(&expresion("a < b == c")), "((a < b) == c)");
}

#[test]
fn valido_lo_aritmetico_tiene_mas_precedencia_que_lo_relacional() {
    assert_eq!(forma(&expresion("1 + 2 < 4")), "((1 + 2) < 4)");
}

#[test]
fn valido_operadores_unarios() {
    assert_eq!(forma(&expresion("-x")), "(-x)");
    assert_eq!(forma(&expresion("!flag")), "(!flag)");
}

#[test]
fn valido_el_unario_liga_mas_fuerte_que_el_binario() {
    assert_eq!(forma(&expresion("-a + b")), "((-a) + b)");
}

#[test]
fn invalido_expresion_incompleta() {
    let salida = errores("fn main(): Void { mut x = 1 + ; }");
    assert!(salida.contains(codes::TOKEN_INESPERADO.as_str()));
}

// --- Condicional ------------------------------------------------------------

#[test]
fn valido_condicional_simple() {
    let Stmt::If(i) = sentencias("if x > 0 { }").remove(0) else {
        panic!("se esperaba un condicional");
    };
    assert!(i.else_branch.is_none());
}

#[test]
fn valido_condicional_con_alternativa() {
    let Stmt::If(i) = sentencias("if x > 0 { } else { }").remove(0) else {
        panic!("se esperaba un condicional");
    };
    assert!(matches!(i.else_branch, Some(ElseBranch::Block(_))));
}

#[test]
fn valido_condicional_encadenado() {
    let Stmt::If(i) = sentencias("if a { } else if b { } else { }").remove(0) else {
        panic!("se esperaba un condicional");
    };
    assert!(matches!(i.else_branch, Some(ElseBranch::If(_))));
}

#[test]
fn invalido_cuerpo_de_condicional_sin_llaves() {
    let salida = errores("fn main(): Void { if x return; }");
    assert!(salida.contains(codes::FALTAN_LLAVES.as_str()));
    assert!(salida.contains("= ayuda:"));
}

// --- Llamadas y retorno -----------------------------------------------------

#[test]
fn valido_llamada_con_argumentos() {
    assert_eq!(forma(&expresion("add(1, 2)")), "add(1, 2)");
}

#[test]
fn valido_llamada_sin_argumentos() {
    assert_eq!(forma(&expresion("ahora()")), "ahora()");
}

#[test]
fn valido_retorno_con_valor() {
    let Stmt::Return(r) = sentencias("return a + b;").remove(0) else {
        panic!("se esperaba un retorno");
    };
    assert!(r.value.is_some());
}

#[test]
fn valido_retorno_sin_valor() {
    let Stmt::Return(r) = sentencias("return;").remove(0) else {
        panic!("se esperaba un retorno");
    };
    assert!(r.value.is_none());
}

#[test]
fn invalido_llamada_sin_cerrar() {
    let salida = errores("fn main(): Void { add(1, 2; }");
    assert!(salida.contains(codes::TOKEN_INESPERADO.as_str()));
}

// --- Asignación -------------------------------------------------------------

#[test]
fn valido_asignacion_a_variable() {
    let Stmt::Assign(a) = sentencias("x = 5;").remove(0) else {
        panic!("se esperaba una asignación");
    };
    assert_eq!(a.target.name, "x");
}

#[test]
fn invalido_asignacion_a_algo_que_no_es_variable() {
    let salida = errores("fn main(): Void { 1 = 5; }");
    assert!(salida.contains(codes::TOKEN_INESPERADO.as_str()));
}

// --- Punto y coma opcional --------------------------------------------------

#[test]
fn valido_sentencias_sin_punto_y_coma() {
    let con = sentencias("mut a = 1; mut b = 2;");
    let sin = sentencias("mut a = 1\n mut b = 2\n");

    assert_eq!(con.len(), sin.len());
    assert_eq!(con.len(), 2);
}

// --- println ----------------------------------------------------------------

#[test]
fn valido_println_con_cadena() {
    assert_eq!(
        forma(&expresion("stdout.println(\"Hola\")")),
        "println(\"Hola\")"
    );
}

#[test]
fn valido_println_con_expresion() {
    assert_eq!(
        forma(&expresion("stdout.println(a + b)")),
        "println((a + b))"
    );
}

#[test]
fn invalido_otro_metodo_de_stdout() {
    let salida = errores("fn main(): Void { stdout.write(\"x\"); }");
    assert!(salida.contains(codes::NO_IMPLEMENTADO.as_str()));
    assert!(salida.contains("Fase 7"));
}

// --- Construcciones de fases posteriores ------------------------------------

#[test]
fn invalido_construcciones_de_otras_fases_dicen_cual() {
    for (fuente, texto, fase) in [
        ("fn main(): Void { for x in y { } }", "for", "Fase 2"),
        ("fn main(): Void { while a { } }", "while", "Fase 2"),
        ("fn main(): Void { match x { } }", "match", "Fase 2"),
        ("class User { }", "class", "Fase 3"),
        ("fn main(): Void { try { } }", "try", "Fase 4"),
        ("fn main(): Void { task { } }", "task", "Fase 5"),
        ("fn main(): Void { parallel { } }", "parallel", "Fase 5"),
    ] {
        let salida = errores(fuente);
        assert!(
            salida.contains(codes::NO_IMPLEMENTADO.as_str()),
            "para `{texto}` faltó el código de no implementado:\n{salida}"
        );
        assert!(
            salida.contains(texto),
            "el diagnóstico debe nombrar `{texto}`:\n{salida}"
        );
        assert!(
            salida.contains(fase),
            "el diagnóstico debe decir `{fase}` para `{texto}`:\n{salida}"
        );
    }
}

#[test]
fn invalido_construcciones_de_otras_fases_no_son_token_inesperado() {
    let salida = errores("class User { }");
    assert!(
        !salida.contains(codes::TOKEN_INESPERADO.as_str()),
        "una construcción conocida no debe reportarse como token inesperado:\n{salida}"
    );
}

#[test]
fn invalido_import_tiene_su_propio_diagnostico() {
    let salida = errores("import { stdout } from std.io;\nfn main(): Void { }");
    assert!(salida.contains(codes::MODULOS_NO_DISPONIBLES.as_str()));
    assert!(salida.contains("Fase 2"));
}

#[test]
fn invalido_asignacion_compuesta_dice_su_fase() {
    let salida = errores("fn main(): Void { mut x = 1; x += 1; }");
    assert!(salida.contains(codes::NO_IMPLEMENTADO.as_str()));
    assert!(salida.contains("+="));
}

// --- Ubicación --------------------------------------------------------------

#[test]
fn valido_todo_nodo_expone_su_span() {
    let source = SourceFile::new("test.zrk", "fn main(): Void { mut x = 42; }");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let p = parse(&source, &tokens, &mut sink);

    let f = &p.functions[0];
    assert_eq!(source.slice(f.name.span), "main");
    assert_eq!(source.slice(f.return_type.span), "Void");

    let Stmt::Let(l) = &f.body.statements[0] else {
        panic!("se esperaba una declaración");
    };
    assert_eq!(source.slice(l.name.span), "x");
    assert_eq!(source.slice(l.init.as_ref().unwrap().span()), "42");
}

// --- Recuperación -----------------------------------------------------------

#[test]
fn invalido_un_error_no_impide_parsear_el_resto() {
    let source = SourceFile::new("test.zrk", "fn a() { }\nfn b(): Void { }");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let p = parse(&source, &tokens, &mut sink);

    assert!(sink.has_errors());
    assert!(
        p.functions.iter().any(|f| f.name.name == "b"),
        "la función válida posterior al error debe parsearse igual"
    );
}

#[test]
fn invalido_el_parser_siempre_termina() {
    // Entradas patológicas: lo que importa es que no cuelguen.
    for fuente in ["fn", "fn main(", "fn main(): { {{{", "}}}", "((((", "= = ="] {
        let source = SourceFile::new("test.zrk", fuente);
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(&source, &mut sink);
        parse(&source, &tokens, &mut sink);
    }
}
