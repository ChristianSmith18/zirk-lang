//! Lowering from the verified tree into IR.
//!
//! The input is a tree whose names are resolved and whose types are checked, so
//! **nothing is re-checked here**. Expression types are derived from the shape
//! of the tree, which is total on a verified program: a literal knows its type,
//! a path reads it from its slot, an operator computes it from its operands,
//! and a call from its signature.
//!
//! Deriving is not checking. Everything the checker would have rejected never
//! reaches this point.

use crate::ir::*;
use std::collections::HashMap;
use zirk_ast as ast;
use zirk_diagnostics::Span;
use zirk_sema::{CheckedProgram, Type};

/// Lowers a verified program into an IR module.
pub fn lower(program: &ast::Program, checked: &CheckedProgram) -> Module {
    let mut module = Module::default();

    for function in &program.functions {
        let lowered = FunctionLowering::new(&mut module, checked).run(function);
        module.functions.push(lowered);
    }

    module
}

/// Converts a frontend type into an IR type.
fn ir_type(ty: Type) -> IrType {
    match ty {
        Type::Void => IrType::Void,
        Type::Int32 => IrType::Int32,
        Type::Boolean => IrType::Boolean,
        Type::String => IrType::String,
        // A verified program contains no `Unknown`: the checker reports and the
        // pipeline stops before reaching lowering.
        Type::Unknown => unreachable!("lowering received an unchecked program"),
    }
}

fn ir_type_from_name(name: &str) -> IrType {
    ir_type(Type::from_name(name).expect("a verified program only names known types"))
}

struct FunctionLowering<'a> {
    module: &'a mut Module,
    checked: &'a CheckedProgram,

    slots: Vec<Slot>,
    blocks: Vec<Block>,
    /// Block instructions are appended to.
    current: BlockId,
    next_value: u32,

    /// Name to slot, per nesting level, to resolve shadowing exactly as the
    /// checker did.
    scopes: Vec<HashMap<String, SlotId>>,
    return_type: IrType,
}

impl<'a> FunctionLowering<'a> {
    fn new(module: &'a mut Module, checked: &'a CheckedProgram) -> Self {
        Self {
            module,
            checked,
            slots: Vec::new(),
            blocks: Vec::new(),
            current: BlockId(0),
            next_value: 0,
            scopes: Vec::new(),
            return_type: IrType::Void,
        }
    }

    // --- Construction helpers ---------------------------------------------

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(Block::new(id));
        id
    }

    fn new_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value);
        self.next_value += 1;
        id
    }

    fn block_mut(&mut self, id: BlockId) -> &mut Block {
        self.blocks
            .iter_mut()
            .find(|b| b.id == id)
            .expect("block created by this lowering")
    }

    /// Appends an instruction that produces a value.
    fn emit(&mut self, kind: InstKind, ty: IrType, span: Span) -> Operand {
        let result = self.new_value();
        let current = self.current;
        self.block_mut(current).instructions.push(Instruction {
            result: Some(result),
            kind,
            ty,
            span,
        });
        Operand(result)
    }

    /// Appends an instruction executed for its effect.
    fn emit_effect(&mut self, kind: InstKind, span: Span) {
        let current = self.current;
        self.block_mut(current).instructions.push(Instruction {
            result: None,
            kind,
            ty: IrType::Void,
            span,
        });
    }

    /// Closes the current block, if it is not closed already.
    ///
    /// Code after a `return` is unreachable and produces no terminator: the
    /// first one wins.
    fn terminate(&mut self, terminator: Terminator) {
        let current = self.current;
        let block = self.block_mut(current);
        if block.terminator.is_none() {
            block.terminator = Some(terminator);
        }
    }

    fn is_terminated(&self, id: BlockId) -> bool {
        self.blocks
            .iter()
            .find(|b| b.id == id)
            .is_some_and(|b| b.terminator.is_some())
    }

    fn declare_slot(&mut self, name: &str, ty: IrType, span: Span) -> SlotId {
        let id = SlotId(self.slots.len() as u32);
        self.slots.push(Slot {
            name: name.to_string(),
            ty,
            span,
        });
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), id);
        }
        id
    }

    fn lookup_slot(&self, name: &str) -> SlotId {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
            .expect("a verified program only names declared variables")
    }

    fn slot_type(&self, id: SlotId) -> IrType {
        self.slots[id.0 as usize].ty
    }

    // --- Function ---------------------------------------------------------

    fn run(mut self, function: &ast::FnDecl) -> Function {
        self.return_type = ir_type_from_name(&function.return_type.name);

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let params: Vec<SlotId> = function
            .params
            .iter()
            .map(|p| {
                let ty = ir_type_from_name(&p.ty.name);
                self.declare_slot(&p.name.name, ty, p.name.span)
            })
            .collect();

        self.lower_block(&function.body);

        // A `Void` function may end without an explicit return; the terminator
        // is added so the IR is well formed. In a non-`Void` function the
        // checker already guaranteed every path returns.
        self.terminate(Terminator::Return(None));

        self.scopes.pop();

        Function {
            name: function.name.name.clone(),
            params,
            return_type: self.return_type,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: function.span,
        }
    }

    fn lower_block(&mut self, block: &ast::Block) {
        self.scopes.push(HashMap::new());
        for stmt in &block.statements {
            self.lower_stmt(stmt);
        }
        self.scopes.pop();
    }

    // --- Statements -------------------------------------------------------

    fn lower_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Let(s) => self.lower_let(s),
            ast::Stmt::Assign(s) => self.lower_assign(s),
            ast::Stmt::If(s) => self.lower_if(s),
            ast::Stmt::Return(s) => self.lower_return(s),
            ast::Stmt::Expr(s) => {
                self.lower_expr_for_effect(&s.expr);
            }
            ast::Stmt::Block(b) => self.lower_block(b),
        }
    }

    fn lower_let(&mut self, stmt: &ast::LetStmt) {
        // The value is computed before declaring the slot so that
        // `mut x = x` — with an outer `x` — reads the outer one, as the checker
        // resolved it.
        let value = stmt.init.as_ref().map(|e| (self.lower_expr(e), e.span()));

        let ty = match &stmt.ty {
            Some(annotation) => ir_type_from_name(&annotation.name),
            None => {
                let (_, span) = value.expect("without a type there is always an initializer");
                self.type_of(stmt.init.as_ref().expect("initializer"), span)
            }
        };

        let slot = self.declare_slot(&stmt.name.name, ty, stmt.name.span);

        if let Some((operand, _)) = value {
            self.emit_effect(InstKind::Store(slot, operand), stmt.span);
        }
    }

    fn lower_assign(&mut self, stmt: &ast::AssignStmt) {
        let value = self.lower_expr(&stmt.value);
        let slot = self.lookup_slot(&stmt.target.name);
        self.emit_effect(InstKind::Store(slot, value), stmt.span);
    }

    /// Lowers `if`/`else` into blocks with a conditional branch.
    ///
    /// ```text
    ///            ┌── condition ──┐
    ///            │   branch      │
    ///            ▼               ▼
    ///          then            else
    ///            │               │
    ///            └──▶ continue ◀─┘
    /// ```
    ///
    /// The continuation block is only created if some branch can reach it: if
    /// both return, creating it would leave an unreachable block with no
    /// terminator.
    fn lower_if(&mut self, stmt: &ast::IfStmt) {
        let condition = self.lower_expr(&stmt.condition);

        let then_block = self.new_block();
        let else_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        self.lower_block(&stmt.then_branch);
        let then_open = !self.is_terminated(self.current);
        let then_end = self.current;

        self.current = else_block;
        match &stmt.else_branch {
            Some(ast::ElseBranch::Block(b)) => self.lower_block(b),
            Some(ast::ElseBranch::If(nested)) => self.lower_if(nested),
            None => {}
        }
        let else_open = !self.is_terminated(self.current);
        let else_end = self.current;

        if !then_open && !else_open {
            // Both branches returned: there is nothing after the conditional.
            return;
        }

        let continue_block = self.new_block();

        if then_open {
            self.current = then_end;
            self.terminate(Terminator::Jump(continue_block));
        }
        if else_open {
            self.current = else_end;
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = continue_block;
    }

    fn lower_return(&mut self, stmt: &ast::ReturnStmt) {
        let value = stmt.value.as_ref().map(|e| self.lower_expr(e));
        self.terminate(Terminator::Return(value));
    }

    // --- Expressions ------------------------------------------------------

    fn lower_expr(&mut self, expr: &ast::Expr) -> Operand {
        let span = expr.span();

        match expr {
            ast::Expr::Int(lit) => {
                self.emit(InstKind::ConstInt(lit.value as i32), IrType::Int32, span)
            }
            ast::Expr::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Expr::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }

            ast::Expr::Path(ident) => {
                let slot = self.lookup_slot(&ident.name);
                let ty = self.slot_type(slot);
                self.emit(InstKind::Load(slot), ty, span)
            }

            ast::Expr::Unary(e) => {
                let operand = self.lower_expr(&e.operand);
                let (op, ty) = match e.op {
                    ast::UnaryOp::Neg => (UnaryOp::Neg, IrType::Int32),
                    ast::UnaryOp::Not => (UnaryOp::Not, IrType::Boolean),
                };
                self.emit(InstKind::Unary { op, operand }, ty, span)
            }

            ast::Expr::Binary(e) => {
                let left = self.lower_expr(&e.left);
                let right = self.lower_expr(&e.right);
                let op = binary_op(e.op);
                let operand_type = self.type_of(&e.left, e.left.span());
                self.emit(
                    InstKind::Binary { op, left, right },
                    op.result_type(operand_type),
                    span,
                )
            }

            ast::Expr::Call(e) => {
                let args = e.args.iter().map(|a| self.lower_expr(a)).collect();
                let returns = self.signature_return(&e.callee.name);
                self.emit(
                    InstKind::Call {
                        callee: e.callee.name.clone(),
                        args,
                    },
                    returns,
                    span,
                )
            }

            ast::Expr::Println(e) => {
                let operand = self.lower_println_argument(e, span);
                self.emit(InstKind::Println(operand), IrType::Void, span)
            }
        }
    }

    /// Lowers an expression whose value is discarded.
    ///
    /// `Println` and `Void` calls produce no value, so emitting a result for
    /// them would leave a value nobody reads.
    fn lower_expr_for_effect(&mut self, expr: &ast::Expr) {
        match expr {
            ast::Expr::Println(e) => {
                let operand = self.lower_println_argument(e, expr.span());
                self.emit_effect(InstKind::Println(operand), expr.span());
            }
            ast::Expr::Call(e) if self.signature_return(&e.callee.name) == IrType::Void => {
                let args = e.args.iter().map(|a| self.lower_expr(a)).collect();
                self.emit_effect(
                    InstKind::Call {
                        callee: e.callee.name.clone(),
                        args,
                    },
                    expr.span(),
                );
            }
            other => {
                self.lower_expr(other);
            }
        }
    }

    /// Lowers the argument of a `println`, converting it when it is not a
    /// `String`.
    ///
    /// `ZIRK_STDLIB_SPEC.md` section 3: every printable value goes through
    /// `to_string()`. Without this the runtime would read an `Int32` as if it
    /// were a pointer.
    fn lower_println_argument(&mut self, expr: &ast::PrintlnExpr, span: Span) -> Operand {
        let operand = self.lower_expr(&expr.arg);
        let ty = self.type_of(&expr.arg, expr.arg.span());

        if ty == IrType::String {
            return operand;
        }

        self.emit(InstKind::ToString(operand), IrType::String, span)
    }

    fn signature_return(&self, name: &str) -> IrType {
        self.checked
            .functions
            .get(name)
            .map(|s| ir_type(s.returns))
            .expect("a verified program only calls declared functions")
    }

    /// Derives the type of an expression from the shape of the tree.
    ///
    /// Total on a verified program. It is derivation, not checking: everything
    /// the checker would have rejected never reaches this point.
    fn type_of(&self, expr: &ast::Expr, _span: Span) -> IrType {
        match expr {
            ast::Expr::Int(_) => IrType::Int32,
            ast::Expr::Bool(_) => IrType::Boolean,
            ast::Expr::Str(_) => IrType::String,
            ast::Expr::Path(ident) => self.slot_type(self.lookup_slot(&ident.name)),
            ast::Expr::Unary(e) => match e.op {
                ast::UnaryOp::Neg => IrType::Int32,
                ast::UnaryOp::Not => IrType::Boolean,
            },
            ast::Expr::Binary(e) => {
                binary_op(e.op).result_type(self.type_of(&e.left, e.left.span()))
            }
            ast::Expr::Call(e) => self.signature_return(&e.callee.name),
            ast::Expr::Println(_) => IrType::Void,
        }
    }
}

fn binary_op(op: ast::BinaryOp) -> BinaryOp {
    use ast::BinaryOp as A;
    match op {
        A::Add => BinaryOp::Add,
        A::Sub => BinaryOp::Sub,
        A::Mul => BinaryOp::Mul,
        A::Div => BinaryOp::Div,
        A::Rem => BinaryOp::Rem,
        A::Eq => BinaryOp::Eq,
        A::NotEq => BinaryOp::NotEq,
        A::Lt => BinaryOp::Lt,
        A::LtEq => BinaryOp::LtEq,
        A::Gt => BinaryOp::Gt,
        A::GtEq => BinaryOp::GtEq,
        A::And => BinaryOp::And,
        A::Or => BinaryOp::Or,
    }
}
