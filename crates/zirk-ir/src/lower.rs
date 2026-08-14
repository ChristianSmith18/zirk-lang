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
use zirk_sema::{Base, CheckedProgram, Type};

/// Lowers a verified program into an IR module.
pub fn lower(program: &ast::Program, checked: &CheckedProgram) -> Module {
    let mut module = Module::default();

    for function in &program.functions {
        let lowering = FunctionLowering::new(&mut module, checked);
        let (lowered, lifted) = lowering.run(function);
        module.functions.push(lowered);
        // Lambdas become module functions of their own: the closure value only
        // carries a pointer to one plus the captures. Decision D10.
        module.functions.extend(lifted);
    }

    module
}

/// Converts a frontend type into an IR type.
fn ir_type(ty: Type) -> IrType {
    let base = match ty.base {
        Base::Void => IrType::Void,
        Base::Int32 => IrType::Int32,
        Base::Boolean => IrType::Boolean,
        Base::String => IrType::String,
        // An enum without associated data is exactly its discriminant. Phase 3
        // gives it a payload and with it a representation of its own.
        Base::Enum(_) => IrType::Int32,
        // A verified program contains none of these: the checker reports and
        // the pipeline stops before reaching lowering.
        Base::Unknown | Base::Null | Base::Function(_) | Base::Range => {
            unreachable!("lowering received a construct the checker should have rejected")
        }
    };

    if !ty.nullable {
        return base;
    }

    IrType::Nullable(Nullable::of(base).expect("the checker rejects `Void?`"))
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
    /// Where `break` and `continue` jump, innermost loop last.
    loops: Vec<LoopTargets>,
    /// Lambda bodies lifted out of the function being lowered.
    lifted: Vec<Function>,
    /// The type of each value emitted, so a destination can ask for it instead
    /// of deriving it from the tree a second time.
    value_types: HashMap<ValueId, IrType>,
}

/// The two blocks a loop exposes to the jumps inside it.
struct LoopTargets {
    break_to: BlockId,
    continue_to: BlockId,
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
            loops: Vec::new(),
            lifted: Vec::new(),
            value_types: HashMap::new(),
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
        self.value_types.insert(result, ty);
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

    /// The IR type a written annotation names.
    ///
    /// Enums are not built in, so the name is resolved against the table the
    /// checker produced rather than against a fixed list.
    fn ir_type_from_ref(&self, reference: &ast::TypeRef) -> IrType {
        let base = if let Some(ty) = Type::from_name(&reference.name) {
            ir_type(ty)
        } else if self.checked.enums.iter().any(|e| e.name == reference.name) {
            IrType::Int32
        } else {
            unreachable!("a verified program only names known types")
        };

        if !reference.nullable {
            return base;
        }
        IrType::Nullable(Nullable::of(base).expect("the checker rejects `Void?`"))
    }

    /// Lowers an expression into the type its destination expects.
    ///
    /// `T` is accepted where `T?` is expected, and `null` fits any nullable
    /// type. Neither is a conversion the IR performs implicitly: this is where
    /// the widening becomes an instruction.
    fn lower_expr_as(&mut self, expr: &ast::Expr, expected: IrType) -> Operand {
        let IrType::Nullable(base) = expected else {
            return self.lower_expr(expr);
        };

        // `null` has no type of its own: the destination supplies it.
        if matches!(expr, ast::Expr::Null(_)) {
            return self.emit(InstKind::NullValue(base), expected, expr.span());
        }

        let value = self.lower_expr(expr);
        if self.type_of(expr, expr.span()) == expected {
            return value;
        }

        self.emit(InstKind::Wrap { base, value }, expected, expr.span())
    }

    // --- Function ---------------------------------------------------------

    fn run(mut self, function: &ast::FnDecl) -> (Function, Vec<Function>) {
        self.return_type = self.ir_type_from_ref(&function.return_type);

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let params: Vec<SlotId> = function
            .params
            .iter()
            .map(|p| {
                let ty = self.ir_type_from_ref(&p.ty);
                self.declare_slot(&p.name.name, ty, p.name.span)
            })
            .collect();

        self.lower_block(&function.body);

        // A `Void` function may end without an explicit return; the terminator
        // is added so the IR is well formed. In a non-`Void` function the
        // checker already guaranteed every path returns.
        self.terminate(Terminator::Return(None));

        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            name: function.name.name.clone(),
            params,
            return_type: self.return_type,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: function.span,
        };

        (lowered, lifted)
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
            ast::Stmt::Loop(s) => self.lower_loop(s),
            ast::Stmt::ForIn(s) => self.lower_for_in(s),
            ast::Stmt::Break(s) => self.lower_break(s),
            ast::Stmt::Continue(s) => self.lower_continue(s),
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
        let annotated = stmt.ty.as_ref().map(|a| self.ir_type_from_ref(a));

        let value = stmt.init.as_ref().map(|e| {
            let operand = match annotated {
                Some(expected) => self.lower_expr_as(e, expected),
                None => self.lower_expr(e),
            };
            (operand, e.span())
        });

        let ty = match (annotated, value) {
            (Some(ty), _) => ty,
            // Without an annotation the type is the one the initializer
            // produced. Asking the value rather than the tree is what lets a
            // lambda be inferred: its closure type only exists once lowered.
            (None, Some((operand, _))) => self.type_of_operand(operand),
            (None, None) => unreachable!("without a type there is always an initializer"),
        };

        let slot = self.declare_slot(&stmt.name.name, ty, stmt.name.span);

        if let Some((operand, _)) = value {
            self.emit_effect(InstKind::Store(slot, operand), stmt.span);
        }
    }

    fn lower_assign(&mut self, stmt: &ast::AssignStmt) {
        let slot = self.lookup_slot(&stmt.target.name);
        let value = self.lower_expr_as(&stmt.value, self.slot_type(slot));
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

    /// Lowers `while`, `loop` and the three-clause `for` into a cycle.
    ///
    /// ```text
    ///     init
    ///      │
    ///      ▼
    ///   ┌ header ──(false)──▶ continue
    ///   │  │(true)
    ///   │  ▼
    ///   │ body ──▶ step ──┐
    ///   └─────────────────┘
    /// ```
    ///
    /// The step is its own block rather than the tail of the body because that
    /// is where `continue` has to jump: skipping it would turn `for` into an
    /// infinite loop the first time someone wrote `continue`.
    fn lower_loop(&mut self, stmt: &ast::LoopStmt) {
        self.scopes.push(HashMap::new());

        if let Some(init) = &stmt.init {
            self.lower_stmt(init);
        }

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        match &stmt.condition {
            Some(condition) => {
                let value = self.lower_expr(condition);
                self.terminate(Terminator::Branch {
                    condition: value,
                    then_block: body_block,
                    else_block: continue_block,
                });
            }
            // `loop` has no exit other than `break`.
            None => self.terminate(Terminator::Jump(body_block)),
        }

        self.loops.push(LoopTargets {
            break_to: continue_block,
            continue_to: step_block,
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        if let Some(step) = &stmt.step {
            self.lower_stmt(step);
        }
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    /// Lowers `for x in a..b` into the counter loop it stands for.
    ///
    /// There is no iterator protocol to call into: `ZIRK_LANGUAGE_SPEC.md`
    /// leaves iteration to traits, which are Phase 3. Over a range the loop is
    /// exactly a counter, so it is built directly instead of inventing a
    /// protocol the language does not define yet. Decision D3.
    fn lower_for_in(&mut self, stmt: &ast::ForInStmt) {
        let ast::Expr::Range(range) = &stmt.iterable else {
            unreachable!("a verified program only iterates ranges in this phase")
        };

        self.scopes.push(HashMap::new());

        let start = self.lower_expr(&range.start);
        let binding = self.declare_slot(&stmt.binding.name, IrType::Int32, stmt.binding.span);
        self.emit_effect(InstKind::Store(binding, start), stmt.span);

        // The end is evaluated once, before the loop: re-evaluating it each
        // iteration would call any function in it repeatedly.
        let end = self.lower_expr(&range.end);
        let limit = self.declare_slot("<range end>", IrType::Int32, range.end.span());
        self.emit_effect(InstKind::Store(limit, end), stmt.span);

        let header = self.new_block();
        let body_block = self.new_block();
        let step_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Jump(header));

        self.current = header;
        let current = self.emit(InstKind::Load(binding), IrType::Int32, stmt.span);
        let bound = self.emit(InstKind::Load(limit), IrType::Int32, stmt.span);
        let op = if range.inclusive {
            BinaryOp::LtEq
        } else {
            BinaryOp::Lt
        };
        let keep_going = self.emit(
            InstKind::Binary {
                op,
                left: current,
                right: bound,
            },
            IrType::Boolean,
            stmt.span,
        );
        self.terminate(Terminator::Branch {
            condition: keep_going,
            then_block: body_block,
            else_block: continue_block,
        });

        self.loops.push(LoopTargets {
            break_to: continue_block,
            continue_to: step_block,
        });

        self.current = body_block;
        self.lower_block(&stmt.body);
        self.terminate(Terminator::Jump(step_block));

        self.loops.pop();

        self.current = step_block;
        let value = self.emit(InstKind::Load(binding), IrType::Int32, stmt.span);
        let one = self.emit(InstKind::ConstInt(1), IrType::Int32, stmt.span);
        let next = self.emit(
            InstKind::Binary {
                op: BinaryOp::Add,
                left: value,
                right: one,
            },
            IrType::Int32,
            stmt.span,
        );
        self.emit_effect(InstKind::Store(binding, next), stmt.span);
        self.terminate(Terminator::Jump(header));

        self.current = continue_block;
        self.scopes.pop();
    }

    fn lower_break(&mut self, _stmt: &ast::JumpStmt) {
        let target = self
            .loops
            .last()
            .expect("a verified program only breaks inside a loop")
            .break_to;
        self.terminate(Terminator::Jump(target));

        // Statements after a `break` are unreachable, but the block they would
        // land in still needs to exist and be terminated.
        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_continue(&mut self, _stmt: &ast::JumpStmt) {
        let target = self
            .loops
            .last()
            .expect("a verified program only continues inside a loop")
            .continue_to;
        self.terminate(Terminator::Jump(target));

        let unreachable = self.new_block();
        self.current = unreachable;
    }

    fn lower_return(&mut self, stmt: &ast::ReturnStmt) {
        let expected = self.return_type;
        let value = stmt.value.as_ref().map(|e| self.lower_expr_as(e, expected));
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

            ast::Expr::Binary(e) if e.op == ast::BinaryOp::Coalesce => self.lower_coalesce(e, span),

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

            // Calling a closure value goes through the value, not a name.
            ast::Expr::Call(e) if self.is_closure_call(e) => {
                let callee = self.lower_expr(&e.callee);
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                let expected = self.module.closures[id as usize].params.clone();
                let returns = self.module.closures[id as usize].returns;
                let args = e
                    .args
                    .iter()
                    .zip(expected)
                    .map(|(a, ty)| self.lower_expr_as(&a.value, ty))
                    .collect();
                self.emit(InstKind::CallClosure { id, callee, args }, returns, span)
            }

            ast::Expr::Call(e) => {
                let name = callee_name(e);
                let args = self.lower_args(e);
                let returns = self.signature_return(name);
                self.emit(
                    InstKind::Call {
                        callee: name.to_string(),
                        args,
                    },
                    returns,
                    span,
                )
            }

            ast::Expr::If(e) => self.lower_if_expr(e, span),
            ast::Expr::Match(e) => self.lower_match(e, span),
            ast::Expr::Variant(e) => {
                let value = self.discriminant(&e.enum_name.name, &e.variant.name);
                self.emit(InstKind::ConstInt(value), IrType::Int32, span)
            }

            ast::Expr::Println(e) => {
                let operand = self.lower_println_argument(e, span);
                self.emit(InstKind::Println(operand), IrType::Void, span)
            }

            // The checker rejects these before lowering runs; see
            // `zirk_sema` and the tasks still open for this phase.
            ast::Expr::Lambda(e) => self.lower_lambda(e, span),

            // `null` has no type of its own: it only appears where a
            // destination supplies one, and `lower_expr_as` handles it there.
            ast::Expr::Null(_) | ast::Expr::Range(_) => {
                unreachable!("lowering received a construct the checker should have rejected")
            }
        }
    }

    /// Lowers `a ?? b` into an explicit null check with two blocks.
    ///
    /// ```text
    ///   is_null(a) ──(yes)──▶ evaluate b ──┐
    ///        │(no)                         │
    ///        ▼                             ▼
    ///   unwrap(a) ────────────────────▶ continue
    /// ```
    ///
    /// The fallback lives in its own block so it is only evaluated when the
    /// value is absent: `a ?? expensive()` must not call `expensive` when `a`
    /// holds a value.
    fn lower_coalesce(&mut self, expr: &ast::BinaryExpr, span: Span) -> Operand {
        let left_type = self.type_of(&expr.left, expr.left.span());
        let right_type = self.type_of(&expr.right, expr.right.span());
        // The fallback decides: when it may itself be absent, so may the
        // result; otherwise the result always holds a value.
        let result_type = if matches!(right_type, IrType::Nullable(_)) {
            right_type
        } else {
            left_type.unwrapped()
        };
        let result = self.declare_slot("<coalesce>", result_type, span);

        let value = self.lower_expr(&expr.left);
        let test = self.emit(InstKind::IsNull(value), IrType::Boolean, span);

        // The value is needed again in the block that unwraps it, and values do
        // not cross blocks (ADR-007).
        let holder = self.declare_slot("<coalesced>", left_type, span);
        self.emit_effect(InstKind::Store(holder, value), span);

        let absent_block = self.new_block();
        let present_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition: test,
            then_block: absent_block,
            else_block: present_block,
        });

        self.current = absent_block;
        let fallback = self.lower_expr_as(&expr.right, result_type);
        self.emit_effect(InstKind::Store(result, fallback), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = present_block;
        let held = self.emit(InstKind::Load(holder), self.slot_type(holder), span);
        let inner = self.emit(InstKind::Unwrap(held), self.slot_type(holder).unwrapped(), span);
        // The result may still be nullable when the fallback is: widening the
        // unwrapped value back keeps both branches storing the same type.
        let inner = match result_type {
            IrType::Nullable(base) => self.emit(InstKind::Wrap { base, value: inner }, result_type, span),
            _ => inner,
        };
        self.emit_effect(InstKind::Store(result, inner), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), result_type, span)
    }

    /// Lowers a lambda into a module function plus a closure value.
    ///
    /// The body becomes a function whose leading parameters are the captures,
    /// and the value pairs a pointer to it with the captured values themselves.
    /// Nothing is allocated: a closure cannot escape in this phase, so the
    /// value lives wherever its slot does. Decision D10.
    fn lower_lambda(&mut self, expr: &ast::LambdaExpr, span: Span) -> Operand {
        let info = self
            .checked
            .lambdas
            .get(&expr.span)
            .expect("the checker records every lambda it accepted");

        let capture_types: Vec<IrType> = info.captures.iter().map(|c| ir_type(c.ty)).collect();
        let param_types: Vec<IrType> = expr.params.iter().map(|p| self.ir_type_from_ref(&p.ty)).collect();
        let returns = self.ir_type_from_ref(&expr.return_type);

        let name = format!("<lambda>#{}", self.module.closures.len());
        let id = self.module.closures.len() as u32;
        self.module.closures.push(ClosureLayout {
            function: name.clone(),
            captures: capture_types.clone(),
            params: param_types.clone(),
            returns,
        });

        // The captures are read in the enclosing function, where their slots
        // live, before the body is lifted out.
        let captures: Vec<Operand> = info
            .captures
            .iter()
            .map(|c| {
                let slot = self.lookup_slot(&c.name);
                self.emit(InstKind::Load(slot), self.slot_type(slot), span)
            })
            .collect();

        let names: Vec<String> = info
            .captures
            .iter()
            .map(|c| c.name.clone())
            .chain(expr.params.iter().map(|p| p.name.name.clone()))
            .collect();
        let types: Vec<IrType> = capture_types.into_iter().chain(param_types).collect();

        let body = self.lift_lambda_body(expr, &name, &names, &types, returns);
        self.lifted.push(body);

        self.emit(InstKind::MakeClosure { id, captures }, IrType::Closure(id), span)
    }

    /// Lowers a lambda body as a function of its own.
    ///
    /// A fresh lowering is used rather than reusing this one: the body has its
    /// own slots and blocks, and sharing them would let a capture resolve to
    /// the enclosing function's slot instead of to the parameter that carries
    /// its copy.
    fn lift_lambda_body(
        &mut self,
        expr: &ast::LambdaExpr,
        name: &str,
        params: &[String],
        types: &[IrType],
        returns: IrType,
    ) -> Function {
        let mut inner = FunctionLowering::new(self.module, self.checked);
        inner.return_type = returns;

        let entry = inner.new_block();
        inner.current = entry;
        inner.scopes.push(HashMap::new());

        let slots: Vec<SlotId> = params
            .iter()
            .zip(types)
            .map(|(name, ty)| inner.declare_slot(name, *ty, expr.span))
            .collect();

        match &*expr.body {
            ast::LambdaBody::Expr(e) => {
                let value = inner.lower_expr_as(e, returns);
                inner.terminate(Terminator::Return(Some(value)));
            }
            ast::LambdaBody::Block(b) => {
                inner.lower_block(b);
                inner.terminate(Terminator::Return(None));
            }
        }

        inner.scopes.pop();

        // A lambda may itself contain lambdas.
        let nested = std::mem::take(&mut inner.lifted);
        self.lifted.extend(nested);

        Function {
            name: name.to_string(),
            params: slots,
            return_type: returns,
            slots: inner.slots,
            blocks: inner.blocks,
            entry,
            span: expr.span,
        }
    }

    /// The discriminant a variant lowers to.
    fn discriminant(&self, enum_name: &str, variant: &str) -> i32 {
        self.checked
            .enums
            .iter()
            .find(|e| e.name == enum_name)
            .and_then(|e| e.discriminant(variant))
            .expect("a verified program only names declared variants") as i32
    }

    /// Lowers a `match` into a chain of comparisons.
    ///
    /// ```text
    ///   test₁ ──(no)──▶ test₂ ──(no)──▶ … ──▶ default
    ///     │(yes)          │(yes)
    ///     ▼               ▼
    ///   arm₁            arm₂ ──▶ continue ◀── …
    /// ```
    ///
    /// A chain rather than a jump table: the backend recognizes the shape and
    /// emits a `switch` when the arms are dense, and building the table here
    /// would duplicate an optimization LLVM already does.
    fn lower_match(&mut self, expr: &ast::MatchExpr, span: Span) -> Operand {
        let scrutinee_type = self.type_of(&expr.scrutinee, expr.scrutinee.span());
        let value = self.lower_expr(&expr.scrutinee);

        // Values do not cross blocks in this IR (ADR-007), and the chain tests
        // the same value in each of them: it goes through a slot, and each
        // block reloads it.
        let scrutinee = self.declare_slot("<scrutinee>", scrutinee_type, span);
        self.emit_effect(InstKind::Store(scrutinee, value), span);

        // A `match` used as a statement produces nothing, and a slot has no
        // `Void` form to hold it.
        let result_type = self.arm_value_type(expr);
        let result = (result_type != IrType::Void)
            .then(|| self.declare_slot("<match>", result_type, span));

        let continue_block = self.new_block();
        let mut reachable = false;

        for arm in &expr.arms {
            // An irrefutable pattern takes the value unconditionally, and
            // anything after it is unreachable.
            let body_block = if arm.pattern.is_irrefutable() {
                let block = self.new_block();
                self.terminate(Terminator::Jump(block));
                block
            } else {
                let test = self.lower_pattern_test(&arm.pattern, scrutinee, scrutinee_type, span);

                let body_block = self.new_block();
                let next_block = self.new_block();
                self.terminate(Terminator::Branch {
                    condition: test,
                    then_block: body_block,
                    else_block: next_block,
                });
                self.current = next_block;
                body_block
            };

            let resume = self.current;
            self.current = body_block;

            self.scopes.push(HashMap::new());
            // A binding pattern names the scrutinee inside its arm.
            if let ast::Pattern::Binding(ident) = &arm.pattern {
                let current = self.emit(InstKind::Load(scrutinee), scrutinee_type, ident.span);
                let slot = self.declare_slot(&ident.name, scrutinee_type, ident.span);
                self.emit_effect(InstKind::Store(slot, current), ident.span);
            }

            let value = match &arm.body {
                ast::ArmBody::Expr(e) => self.lower_expr(e),
                ast::ArmBody::Block(b) if result_type == IrType::Void => {
                    self.lower_block(b);
                    self.emit(InstKind::ConstInt(0), IrType::Int32, span)
                }
                ast::ArmBody::Block(b) => self.lower_block_value(b),
            };
            self.scopes.pop();

            if let Some(slot) = result {
                self.emit_effect(InstKind::Store(slot, value), span);
            }
            self.terminate(Terminator::Jump(continue_block));
            reachable = true;

            self.current = resume;
            if arm.pattern.is_irrefutable() {
                break;
            }
        }

        // The checker proved the arms exhaustive, so falling off the chain
        // cannot happen at run time — but the block still needs a terminator
        // for the IR to be well formed.
        if !reachable || !self.is_terminated(self.current) {
            self.terminate(Terminator::Jump(continue_block));
        }

        self.current = continue_block;

        match result {
            Some(slot) => self.emit(InstKind::Load(slot), result_type, span),
            // Nothing reads the result of a statement `match`; a placeholder
            // keeps the signature of `lower_expr` total.
            None => self.emit(InstKind::ConstInt(0), IrType::Int32, span),
        }
    }

    /// The comparison one pattern stands for.
    fn lower_pattern_test(
        &mut self,
        pattern: &ast::Pattern,
        scrutinee: SlotId,
        scrutinee_type: IrType,
        span: Span,
    ) -> Operand {
        let left = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);

        let expected = match pattern {
            ast::Pattern::Int(lit) => {
                self.emit(InstKind::ConstInt(lit.value as i32), IrType::Int32, span)
            }
            ast::Pattern::Bool(lit) => {
                self.emit(InstKind::ConstBool(lit.value), IrType::Boolean, span)
            }
            ast::Pattern::Str(lit) => {
                let id = self.module.intern_string(&lit.value);
                self.emit(InstKind::ConstString(id), IrType::String, span)
            }
            ast::Pattern::Variant(v) => {
                let value = self.discriminant(&v.enum_name.name, &v.variant.name);
                self.emit(InstKind::ConstInt(value), IrType::Int32, span)
            }
            ast::Pattern::Wildcard(_) | ast::Pattern::Binding(_) | ast::Pattern::Null(_) => {
                unreachable!("an irrefutable or null pattern is not tested this way")
            }
        };

        self.emit(
            InstKind::Binary {
                op: BinaryOp::Eq,
                left,
                right: expected,
            },
            IrType::Boolean,
            span,
        )
    }

    /// The type the arms of a `match` produce.
    fn arm_value_type(&self, expr: &ast::MatchExpr) -> IrType {
        for arm in &expr.arms {
            match &arm.body {
                ast::ArmBody::Expr(e) => return self.type_of(e, e.span()),
                ast::ArmBody::Block(b) => {
                    if matches!(b.statements.last(), Some(ast::Stmt::Expr(_))) {
                        return self.block_value_type(b);
                    }
                    return IrType::Void;
                }
            }
        }
        IrType::Void
    }

    /// Lowers an `if` used as a value.
    ///
    /// The branches write into one slot and the continuation reads it. With
    /// locals as slots and no SSA of our own (ADR-007), that is the whole of
    /// it: LLVM promotes the slot to a register and inserts the phi node.
    fn lower_if_expr(&mut self, stmt: &ast::IfStmt, span: Span) -> Operand {
        let ty = self.block_value_type(&stmt.then_branch);
        let result = self.declare_slot("<if>", ty, span);

        let condition = self.lower_expr(&stmt.condition);
        let then_block = self.new_block();
        let else_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        });

        self.current = then_block;
        let value = self.lower_block_value(&stmt.then_branch);
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = else_block;
        let value = match &stmt.else_branch {
            Some(ast::ElseBranch::Block(b)) => self.lower_block_value(b),
            Some(ast::ElseBranch::If(nested)) => self.lower_if_expr(nested, span),
            None => unreachable!("a verified `if` expression always has an `else`"),
        };
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), ty, span)
    }

    /// Lowers a block used as a value: its statements, then its last
    /// expression.
    fn lower_block_value(&mut self, block: &ast::Block) -> Operand {
        self.scopes.push(HashMap::new());

        let (last, rest) = block
            .statements
            .split_last()
            .expect("a verified block used as a value is not empty");

        for stmt in rest {
            self.lower_stmt(stmt);
        }

        let ast::Stmt::Expr(e) = last else {
            unreachable!("a verified block used as a value ends in an expression")
        };
        let value = self.lower_expr(&e.expr);

        self.scopes.pop();
        value
    }

    /// The type a block used as a value produces.
    fn block_value_type(&self, block: &ast::Block) -> IrType {
        let Some(ast::Stmt::Expr(e)) = block.statements.last() else {
            unreachable!("a verified block used as a value ends in an expression")
        };
        self.type_of(&e.expr, e.expr.span())
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
            // A closure call goes through the value and has its own arm in
            // `lower_expr`; only a direct call is special-cased here.
            ast::Expr::Call(e)
                if !self.is_closure_call(e)
                    && self.signature_return(callee_name(e)) == IrType::Void =>
            {
                let name = callee_name(e).to_string();
                let args = self.lower_args(e);
                self.emit_effect(InstKind::Call { callee: name, args }, expr.span());
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

    /// Lowers the arguments of a call into the parameter types.
    fn lower_args(&mut self, call: &ast::CallExpr) -> Vec<Operand> {
        let expected: Vec<IrType> = self
            .checked
            .functions
            .get(callee_name(call))
            .map(|s| s.params.iter().map(|p| ir_type(p.ty)).collect())
            .expect("a verified program only calls declared functions");

        call.args
            .iter()
            .zip(expected)
            .map(|(a, ty)| self.lower_expr_as(&a.value, ty))
            .collect()
    }

    /// Whether a call goes through a closure value rather than a name.
    fn is_closure_call(&self, call: &ast::CallExpr) -> bool {
        match &*call.callee {
            ast::Expr::Path(ident) => self
                .scopes
                .iter()
                .rev()
                .find_map(|scope| scope.get(&ident.name))
                .is_some_and(|slot| matches!(self.slot_type(*slot), IrType::Closure(_))),
            other => matches!(self.type_of(other, other.span()), IrType::Closure(_)),
        }
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
            // `??` yields its operands' shared type, not a boolean or an
            // arithmetic result: it is the one binary operator that is not an
            // operator in the IR at all.
            ast::Expr::Binary(e) if e.op == ast::BinaryOp::Coalesce => {
                let left = self.type_of(&e.left, e.left.span()).unwrapped();
                let right = self.type_of(&e.right, e.right.span());
                if matches!(right, IrType::Nullable(_)) {
                    right
                } else {
                    left
                }
            }
            ast::Expr::Binary(e) => {
                binary_op(e.op).result_type(self.type_of(&e.left, e.left.span()))
            }
            ast::Expr::Call(e) if self.is_closure_call(e) => {
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                self.module.closures[id as usize].returns
            }
            ast::Expr::Call(e) => self.signature_return(callee_name(e)),
            ast::Expr::If(e) => self.block_value_type(&e.then_branch),
            ast::Expr::Match(e) => self.arm_value_type(e),
            ast::Expr::Variant(_) => IrType::Int32,
            ast::Expr::Println(_) => IrType::Void,
            // A lambda's type is the closure layout it produced, which only
            // exists once it has been lowered: the caller asks the value.
            ast::Expr::Lambda(_) | ast::Expr::Null(_) | ast::Expr::Range(_) => {
                unreachable!("the type of this expression comes from the value it produced")
            }
        }
    }

    /// The type of an already emitted value.
    fn type_of_operand(&self, operand: Operand) -> IrType {
        self.value_types
            .get(&operand.0)
            .copied()
            .expect("every emitted value records its type")
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
        // `??` is expanded by the lowering into a null check with two blocks,
        // so it never reaches the IR as an operator.
        A::Coalesce => unreachable!("`??` is lowered into branches, not an operator"),
    }
}

/// The name of a directly called function.
///
/// Calling a closure value goes through a different instruction, and the
/// checker rejects anything else, so a verified program only reaches here with
/// a plain name.
fn callee_name(call: &ast::CallExpr) -> &str {
    match &*call.callee {
        ast::Expr::Path(ident) => &ident.name,
        _ => unreachable!("a verified program calls a name or a closure value"),
    }
}
