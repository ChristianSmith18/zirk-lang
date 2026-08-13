//! Parser descendente recursivo con precedencia por escalada.
//!
//! La precedencia se implementa con un bucle sobre niveles en vez de una
//! función por nivel: la tabla de `nivel()` es la definición legible de lo que
//! `ZIRK_LANGUAGE_SPEC.md` sección 4 fija, y agregar un operador es agregar una
//! fila y no una función.

use crate::codes;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};
use zirk_lexer::{Keyword, Token, TokenKind};

/// Parsea una secuencia de tokens en un programa.
///
/// Devuelve el árbol aunque haya errores: quien llama decide si continuar,
/// consultando `sink.has_errors()`.
pub fn parse(source: &SourceFile, tokens: &[Token], sink: &mut DiagnosticSink) -> Program {
    Parser::new(source, tokens, sink).parse_program()
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: &'a [Token],
    sink: &'a mut DiagnosticSink,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a SourceFile, tokens: &'a [Token], sink: &'a mut DiagnosticSink) -> Self {
        Self {
            source,
            tokens,
            sink,
            pos: 0,
        }
    }

    // --- Navegación -------------------------------------------------------

    fn peek(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_else(|| Span::empty(self.source.text().len() as u32))
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    /// Consume el token si coincide con la clase esperada.
    fn eat(&mut self, esperado: &TokenKind) -> bool {
        if self.peek() == esperado {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, k: Keyword) -> bool {
        self.eat(&TokenKind::Keyword(k))
    }

    fn check_keyword(&self, k: Keyword) -> bool {
        self.peek() == &TokenKind::Keyword(k)
    }

    // --- Diagnósticos -----------------------------------------------------

    fn emitir(&mut self, code: Code, span: Span, mensaje: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, mensaje)
            .at(self.source.location(span))
            .with_snippet(self.source.snippet(span))
    }

    fn error_con(
        &mut self,
        code: Code,
        span: Span,
        mensaje: impl Into<String>,
        causa: impl Into<String>,
        ayuda: Option<String>,
    ) {
        let mut d = self.emitir(code, span, mensaje).with_cause(causa);
        if let Some(ayuda) = ayuda {
            d = d.with_help(ayuda);
        }
        self.sink.emit(d);
    }

    /// Emite el diagnóstico de construcción no implementada, si corresponde.
    ///
    /// Devuelve `true` cuando el token actual es una construcción de una fase
    /// posterior, para que quien llama abandone esa rama en vez de intentar
    /// parsearla.
    fn reportar_si_es_de_otra_fase(&mut self) -> bool {
        let span = self.peek_span();

        // Los módulos tienen su propio diagnóstico: es la pregunta más
        // frecuente de quien prueba el lenguaje por primera vez.
        if self.check_keyword(Keyword::Import)
            || self.check_keyword(Keyword::Share)
            || self.check_keyword(Keyword::Use)
        {
            self.error_con(
                codes::MODULOS_NO_DISPONIBLES,
                span,
                "los módulos multi-archivo todavía no están disponibles",
                "esta versión del compilador procesa un único archivo",
                Some(
                    "escribí todo en un archivo por ahora; `import` y `share` llegan en la Fase 2"
                        .into(),
                ),
            );
            self.sincronizar();
            return true;
        }

        let pendiente = match self.peek() {
            TokenKind::Keyword(k) => k.fase().map(|f| (k.as_str().to_string(), f)),
            otro => otro.fase().map(|f| (otro.simbolo().to_string(), f)),
        };

        let Some((texto, fase)) = pendiente else {
            return false;
        };

        self.error_con(
            codes::NO_IMPLEMENTADO,
            span,
            format!("`{texto}` todavía no está implementado"),
            format!("la construcción existe en el lenguaje pero llega en la Fase {fase}"),
            Some("consultá docs/init/ZIRK_ROADMAP.md para ver el alcance de cada fase".into()),
        );
        self.sincronizar();
        true
    }

    /// Avanza hasta un punto donde tenga sentido retomar el parseo.
    ///
    /// Sin esto, un error produce una cascada de errores derivados que oculta
    /// el problema real.
    fn sincronizar(&mut self) {
        // Se saltan los bloques balanceados enteros. Sin esto, reportar `class`
        // dejaba el `}` de su cuerpo suelto, y ese `}` producía un segundo
        // diagnóstico —"se esperaba una declaración de función"— que no
        // corresponde a ningún error del usuario.
        let mut profundidad = 0usize;

        while !self.at_eof() {
            match self.peek() {
                TokenKind::LBrace => {
                    profundidad += 1;
                    self.pos += 1;
                }
                TokenKind::RBrace => {
                    if profundidad == 0 {
                        // Cierra un bloque que abrió quien nos llamó: es suyo.
                        return;
                    }
                    profundidad -= 1;
                    self.pos += 1;
                    if profundidad == 0 {
                        return;
                    }
                }
                TokenKind::Semicolon if profundidad == 0 => {
                    self.pos += 1;
                    return;
                }
                TokenKind::Keyword(Keyword::Fn) if profundidad == 0 => return,
                _ => self.pos += 1,
            }
        }
    }

    /// Consume el token esperado o emite un diagnóstico.
    fn esperar(&mut self, esperado: &TokenKind, contexto: &str) -> bool {
        if self.eat(esperado) {
            return true;
        }

        let span = self.peek_span();
        let encontrado = self.peek().descripcion();
        self.error_con(
            codes::TOKEN_INESPERADO,
            span,
            format!("se esperaba `{}` {contexto}", esperado.simbolo()),
            format!("se encontró {encontrado}"),
            None,
        );
        false
    }

    fn esperar_identificador(&mut self, contexto: &str) -> Option<Ident> {
        let span = self.peek_span();
        if let TokenKind::Identifier(nombre) = self.peek().clone() {
            self.pos += 1;
            return Some(Ident::new(nombre, span));
        }

        let encontrado = self.peek().descripcion();
        self.error_con(
            codes::TOKEN_INESPERADO,
            span,
            format!("se esperaba un identificador {contexto}"),
            format!("se encontró {encontrado}"),
            None,
        );
        None
    }

    // --- Programa ---------------------------------------------------------

    fn parse_program(mut self) -> Program {
        let inicio = self.peek_span();
        let mut functions = Vec::new();

        while !self.at_eof() {
            if self.check_keyword(Keyword::Fn) {
                if let Some(f) = self.parse_fn() {
                    functions.push(f);
                }
                continue;
            }

            if self.reportar_si_es_de_otra_fase() {
                continue;
            }

            let span = self.peek_span();
            let encontrado = self.peek().descripcion();
            self.error_con(
                codes::TOKEN_INESPERADO,
                span,
                "se esperaba una declaración de función",
                format!("se encontró {encontrado} en el nivel superior del archivo"),
                Some("en esta fase un archivo solo contiene funciones".into()),
            );
            self.sincronizar();
            if !self.check_keyword(Keyword::Fn) && !self.at_eof() {
                self.pos += 1;
            }
        }

        let fin = self.peek_span();
        Program {
            functions,
            span: inicio.to(fin),
        }
    }

    fn parse_fn(&mut self) -> Option<FnDecl> {
        let inicio = self.peek_span();
        self.eat_keyword(Keyword::Fn);

        let name = self.esperar_identificador("después de `fn`")?;

        self.esperar(&TokenKind::LParen, "después del nombre de la función");
        let params = self.parse_params();
        self.esperar(&TokenKind::RParen, "para cerrar los parámetros");

        // El tipo de retorno es obligatorio en esta fase.
        let return_type = if self.eat(&TokenKind::Colon) {
            self.parse_type()?
        } else {
            let span = self.peek_span();
            self.error_con(
                codes::FALTA_TIPO_RETORNO,
                span,
                "falta el tipo de retorno de la función",
                "toda función declara su tipo de retorno",
                Some(format!(
                    "escribí `fn {}(...): Void` si la función no devuelve nada",
                    name.name
                )),
            );
            self.sincronizar();
            return None;
        };

        let body = self.parse_block()?;
        let span = inicio.to(body.span);

        Some(FnDecl {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();

        while !matches!(self.peek(), TokenKind::RParen) && !self.at_eof() {
            let inicio = self.peek_span();

            let Some(name) = self.esperar_identificador("como nombre de parámetro") else {
                break;
            };
            if !self.esperar(&TokenKind::Colon, "después del nombre del parámetro") {
                break;
            }
            let Some(ty) = self.parse_type() else { break };

            let span = inicio.to(ty.span);
            params.push(Param { name, ty, span });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        params
    }

    fn parse_type(&mut self) -> Option<TypeRef> {
        let span = self.peek_span();
        if let TokenKind::Identifier(nombre) = self.peek().clone() {
            self.pos += 1;
            return Some(TypeRef::new(nombre, span));
        }

        let encontrado = self.peek().descripcion();
        self.error_con(
            codes::TOKEN_INESPERADO,
            span,
            "se esperaba un tipo",
            format!("se encontró {encontrado}"),
            Some("los tipos disponibles son Void, Int32, Boolean y String".into()),
        );
        None
    }

    // --- Sentencias -------------------------------------------------------

    fn parse_block(&mut self) -> Option<Block> {
        let inicio = self.peek_span();

        if !self.eat(&TokenKind::LBrace) {
            let encontrado = self.peek().descripcion();
            self.error_con(
                codes::FALTAN_LLAVES,
                inicio,
                "se esperaba un bloque entre llaves",
                format!("se encontró {encontrado}"),
                Some("los cuerpos siempre van entre `{` y `}`".into()),
            );
            return None;
        }

        let mut statements = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let antes = self.pos;

            if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            }

            // Garantía de progreso: si una rama de error no consumió nada,
            // avanzar evita un bucle infinito.
            if self.pos == antes {
                self.pos += 1;
            }
        }

        let fin = self.peek_span();
        self.esperar(&TokenKind::RBrace, "para cerrar el bloque");

        Some(Block {
            statements,
            span: inicio.to(fin),
        })
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        if self.check_keyword(Keyword::Mut) || self.check_keyword(Keyword::Inmut) {
            return self.parse_let();
        }
        if self.check_keyword(Keyword::If) {
            return self.parse_if().map(Stmt::If);
        }
        if self.check_keyword(Keyword::Return) {
            return self.parse_return();
        }
        if matches!(self.peek(), TokenKind::LBrace) {
            return self.parse_block().map(Stmt::Block);
        }
        if self.reportar_si_es_de_otra_fase() {
            return None;
        }

        self.parse_expr_or_assign()
    }

    fn parse_let(&mut self) -> Option<Stmt> {
        let inicio = self.peek_span();
        let mutability = if self.eat_keyword(Keyword::Mut) {
            Mutability::Mutable
        } else {
            self.eat_keyword(Keyword::Inmut);
            Mutability::Immutable
        };

        // `inmut::strict` es de una fase posterior.
        if matches!(self.peek(), TokenKind::ColonColon) {
            let span = self.peek_span();
            self.error_con(
                codes::NO_IMPLEMENTADO,
                span,
                "`inmut::strict` todavía no está implementado",
                "la inmutabilidad profunda llega en una fase posterior",
                Some("usá `inmut` por ahora".into()),
            );
            self.sincronizar();
            return None;
        }

        let name = self.esperar_identificador("después de `mut` o `inmut`")?;

        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.eat(&TokenKind::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        // Sin tipo ni inicializador no hay forma de saber de qué tipo es.
        if ty.is_none() && init.is_none() {
            let span = inicio.to(name.span);
            self.error_con(
                codes::DECLARACION_SIN_TIPO,
                span,
                format!("no se puede determinar el tipo de `{}`", name.name),
                "la declaración no tiene anotación de tipo ni valor inicial",
                Some(format!(
                    "escribí `{}: Int32` o dale un valor inicial",
                    name.name
                )),
            );
        }

        let fin = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Let(LetStmt {
            mutability,
            name,
            ty,
            init,
            span: inicio.to(fin),
        }))
    }

    fn parse_if(&mut self) -> Option<IfStmt> {
        let inicio = self.peek_span();
        self.eat_keyword(Keyword::If);

        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.eat_keyword(Keyword::Else) {
            if self.check_keyword(Keyword::If) {
                Some(ElseBranch::If(Box::new(self.parse_if()?)))
            } else {
                Some(ElseBranch::Block(self.parse_block()?))
            }
        } else {
            None
        };

        let fin = match &else_branch {
            Some(ElseBranch::Block(b)) => b.span,
            Some(ElseBranch::If(i)) => i.span,
            None => then_branch.span,
        };

        Some(IfStmt {
            condition,
            then_branch,
            else_branch,
            span: inicio.to(fin),
        })
    }

    fn parse_return(&mut self) -> Option<Stmt> {
        let inicio = self.peek_span();
        self.eat_keyword(Keyword::Return);

        let value = if matches!(self.peek(), TokenKind::Semicolon | TokenKind::RBrace) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        let fin = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Return(ReturnStmt {
            value,
            span: inicio.to(fin),
        }))
    }

    /// Una sentencia que empieza por expresión: puede ser asignación o llamada.
    fn parse_expr_or_assign(&mut self) -> Option<Stmt> {
        let inicio = self.peek_span();
        let expr = self.parse_expr()?;

        if matches!(self.peek(), TokenKind::Assign) {
            self.pos += 1;
            let value = self.parse_expr()?;
            let fin = self.peek_span();
            self.eat(&TokenKind::Semicolon);

            let Expr::Path(target) = expr else {
                self.error_con(
                    codes::TOKEN_INESPERADO,
                    inicio,
                    "el lado izquierdo de una asignación debe ser una variable",
                    "solo se puede asignar a un nombre",
                    None,
                );
                return None;
            };

            return Some(Stmt::Assign(AssignStmt {
                target,
                value,
                span: inicio.to(fin),
            }));
        }

        let fin = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Expr(ExprStmt {
            span: inicio.to(fin),
            expr,
        }))
    }

    // --- Expresiones ------------------------------------------------------

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_binary(0)
    }

    /// Escalada de precedencia.
    ///
    /// Todos los operadores del subset son asociativos por la izquierda, así que
    /// el nivel del lado derecho es siempre `nivel + 1`.
    fn parse_binary(&mut self, nivel_minimo: u8) -> Option<Expr> {
        let mut left = self.parse_unary()?;

        loop {
            let Some((op, nivel)) = nivel(self.peek()) else {
                break;
            };
            if nivel < nivel_minimo {
                break;
            }

            let op_span = self.peek_span();
            self.pos += 1;

            let right = self.parse_binary(nivel + 1)?;
            let span = left.span().to(right.span());

            left = Expr::Binary(BinaryExpr {
                op,
                left: Box::new(left),
                right: Box::new(right),
                op_span,
                span,
            });
        }

        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let inicio = self.peek_span();

        let op = match self.peek() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Not => Some(UnaryOp::Not),
            _ => None,
        };

        if let Some(op) = op {
            self.pos += 1;
            let operand = self.parse_unary()?;
            let span = inicio.to(operand.span());
            return Some(Expr::Unary(UnaryExpr {
                op,
                operand: Box::new(operand),
                span,
            }));
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let span = self.peek_span();

        match self.peek().clone() {
            TokenKind::Integer(value) => {
                self.pos += 1;
                Some(Expr::Int(IntLit { value, span }))
            }
            TokenKind::Str(value) => {
                self.pos += 1;
                Some(Expr::Str(StrLit { value, span }))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.pos += 1;
                Some(Expr::Bool(BoolLit { value: true, span }))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.pos += 1;
                Some(Expr::Bool(BoolLit { value: false, span }))
            }
            TokenKind::LParen => {
                self.pos += 1;
                let expr = self.parse_expr()?;
                self.esperar(&TokenKind::RParen, "para cerrar el paréntesis");
                Some(expr)
            }
            TokenKind::Identifier(nombre) => {
                self.pos += 1;
                self.parse_after_ident(Ident::new(nombre, span))
            }
            _ => {
                if self.reportar_si_es_de_otra_fase() {
                    return None;
                }
                let encontrado = self.peek().descripcion();
                self.error_con(
                    codes::TOKEN_INESPERADO,
                    span,
                    "se esperaba una expresión",
                    format!("se encontró {encontrado}"),
                    None,
                );
                None
            }
        }
    }

    /// Lo que puede seguir a un identificador: una llamada, `stdout.println`, o
    /// nada.
    fn parse_after_ident(&mut self, ident: Ident) -> Option<Expr> {
        // `stdout.println(expr)` es una forma sintáctica especial mientras no
        // existan módulos ni stdlib. Ver decisión D4 del design.
        if ident.name == "stdout" && matches!(self.peek(), TokenKind::Dot) {
            self.pos += 1;
            let metodo_span = self.peek_span();
            let metodo = self.esperar_identificador("después de `stdout.`")?;

            if metodo.name != "println" {
                self.error_con(
                    codes::NO_IMPLEMENTADO,
                    metodo_span,
                    format!("`stdout.{}` todavía no está disponible", metodo.name),
                    "en esta fase solo existe `stdout.println`",
                    Some("la biblioteca estándar completa llega en la Fase 7".into()),
                );
                return None;
            }

            self.esperar(&TokenKind::LParen, "después de `println`");
            let arg = self.parse_expr()?;
            let fin = self.peek_span();
            self.esperar(&TokenKind::RParen, "para cerrar la llamada");

            return Some(Expr::Println(PrintlnExpr {
                arg: Box::new(arg),
                span: ident.span.to(fin),
            }));
        }

        if matches!(self.peek(), TokenKind::LParen) {
            self.pos += 1;
            let mut args = Vec::new();

            while !matches!(self.peek(), TokenKind::RParen) && !self.at_eof() {
                args.push(self.parse_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }

            let fin = self.peek_span();
            self.esperar(&TokenKind::RParen, "para cerrar los argumentos");

            return Some(Expr::Call(CallExpr {
                span: ident.span.to(fin),
                callee: ident,
                args,
            }));
        }

        Some(Expr::Path(ident))
    }
}

/// Operador binario y su nivel de precedencia.
///
/// Mayor número, mayor precedencia. Es la traducción directa de la tabla de
/// `ZIRK_LANGUAGE_SPEC.md` sección 4.
fn nivel(kind: &TokenKind) -> Option<(BinaryOp, u8)> {
    use BinaryOp::*;
    use TokenKind as T;

    Some(match kind {
        T::OrOr => (Or, 1),
        T::AndAnd => (And, 2),
        T::Eq => (BinaryOp::Eq, 3),
        T::NotEq => (NotEq, 3),
        T::Lt => (Lt, 4),
        T::LtEq => (LtEq, 4),
        T::Gt => (Gt, 4),
        T::GtEq => (GtEq, 4),
        T::Plus => (Add, 5),
        T::Minus => (Sub, 5),
        T::Star => (Mul, 6),
        T::Slash => (Div, 6),
        T::Percent => (Rem, 6),
        _ => return None,
    })
}
