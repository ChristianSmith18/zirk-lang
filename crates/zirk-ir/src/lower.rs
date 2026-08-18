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
    // The layouts come first: a function that builds an object needs the
    // layout it allocates, and a field access needs its offsets.
    let objects = checked
        .classes
        .iter()
        .map(|class| ObjectLayout {
            name: class.name.clone(),
            fields: class
                .fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    ty: ir_type(field.ty),
                })
                .collect(),
            // Ordered by index, which is what makes the table of a subclass
            // start with its base's.
            methods: {
                let mut table = vec![String::new(); class.methods.len()];
                for method in &class.methods {
                    table[method.index] = body_symbol(checked, method);
                }
                table
            },
            // One table per contract, in the contract's own method order, so
            // a call through it indexes the same way whatever class answers.
            contracts: class
                .contracts
                .iter()
                .map(|id| ContractTable {
                    contract: *id,
                    methods: checked.contracts[*id as usize]
                        .methods
                        .iter()
                        .map(|required| {
                            let supplied = class
                                .method(&required.name)
                                .expect("the checker verified conformance");
                            body_symbol(checked, supplied)
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    let mut module = Module {
        objects,
        ..Module::default()
    };

    // Default values are written in the declaration but evaluated at the call
    // site, so the lowering needs the declarations while lowering the calls.
    let declarations: HashMap<&str, &ast::FnDecl> = program
        .functions
        .iter()
        .map(|f| (f.name.name.as_str(), f))
        .collect();

    // A constructor becomes an ordinary function whose first parameter is the
    // object being built. Emitting it once and calling it beats inlining the
    // body at every construction site, and it is the same shape a method call
    // will need.
    for class in &program.classes {
        let Some(id) = checked
            .classes
            .iter()
            .position(|c| c.name == class.name.name)
        else {
            continue;
        };
        for (index, constructor) in class.constructors.iter().enumerate() {
            let lowering = FunctionLowering::new(&mut module, checked, &declarations);
            let (lowered, lifted) = lowering.run_constructor(class, constructor, id as u32, index);
            module.functions.push(lowered);
            module.functions.extend(lifted);
        }

        // A method is the same shape as a constructor: a function whose first
        // parameter is the receiver.
        for (index, method) in class.methods.iter().enumerate() {
            let Some(body) = &method.body else { continue };
            let lowering = FunctionLowering::new(&mut module, checked, &declarations);
            let (lowered, lifted) = lowering.run_method(class, method, body, id as u32, index);
            module.functions.push(lowered);
            module.functions.extend(lifted);
        }
    }

    // A trait's default body belongs to the contract, not to any class that
    // adopts it: one body, however many classes reuse it.
    for contract in &program.contracts {
        let Some(id) = checked
            .contracts
            .iter()
            .position(|c| c.name == contract.name.name)
        else {
            continue;
        };
        for method in &contract.methods {
            let Some(body) = &method.body else { continue };
            let lowering = FunctionLowering::new(&mut module, checked, &declarations);
            let (lowered, lifted) =
                lowering.run_contract_default(contract, method, body, id as u32);
            module.functions.push(lowered);
            module.functions.extend(lifted);
        }
    }

    for function in &program.functions {
        let lowering = FunctionLowering::new(&mut module, checked, &declarations);
        let (lowered, lifted) = lowering.run(function);
        module.functions.push(lowered);
        // Lambdas become module functions of their own: the closure value only
        // carries a pointer to one plus the captures. Decision D10.
        module.functions.extend(lifted);
    }

    module
}

/// The name a constructor is emitted under.
///
/// The class name plus the index of the `construct` in its declaration order,
/// because a class may have several and they must not collide. The `$` cannot
/// appear in a Zirk identifier, so a user function can never take the name.
pub fn constructor_symbol(class: &str, index: usize) -> String {
    format!("{class}$construct${index}")
}

/// The name a method is emitted under.
///
/// Its own name suffices: a class cannot declare two methods with the same
/// one, since there is no overloading.
pub fn method_symbol(class: &str, method: &str) -> String {
    format!("{class}${method}")
}

/// The name a trait's own default body is emitted under.
///
/// It belongs to the contract, not to any class that adopts it: one body,
/// however many classes reuse it.
pub fn contract_method_symbol(contract: &str, method: &str) -> String {
    format!("{contract}$default${method}")
}

/// The symbol whose body a method entry actually reaches.
///
/// A class that adopted a trait's default has an entry of its own, but the
/// body belongs to the trait: one body, however many classes reuse it.
fn body_symbol(checked: &CheckedProgram, method: &zirk_sema::MethodInfo) -> String {
    match method.from_contract {
        Some(contract) => {
            contract_method_symbol(&checked.contracts[contract as usize].name, &method.name)
        }
        None => method_symbol(&checked.classes[method.owner as usize].name, &method.name),
    }
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
        // An object is reached through its address: that is its identity.
        Base::Class(id) => IrType::Object(id),
        Base::Contract(id) => IrType::Contract(id),
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
    declarations: &'a HashMap<&'a str, &'a ast::FnDecl>,

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
    fn new(
        module: &'a mut Module,
        checked: &'a CheckedProgram,
        declarations: &'a HashMap<&'a str, &'a ast::FnDecl>,
    ) -> Self {
        Self {
            module,
            checked,
            declarations,
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
    /// The declaration a call names, with any import alias already applied.
    ///
    /// The checker recorded the resolution, so lowering does not need to know
    /// that aliases exist.
    fn callee_name(&self, call: &ast::CallExpr) -> String {
        let ident = callee_ident(call);
        self.declaration_of(&ident.name, ident.span)
    }

    /// The declaration a written name refers to, following import aliases.
    fn declaration_of(&self, written: &str, at: Span) -> String {
        self.checked
            .aliases
            .get(&at)
            .cloned()
            .unwrap_or_else(|| written.to_string())
    }

    fn ir_type_from_ref(&self, reference: &ast::TypeRef) -> IrType {
        let declared = self.declaration_of(&reference.name, reference.span);
        let base = if let Some(ty) = Type::from_name(&reference.name) {
            ir_type(ty)
        } else if self.checked.enums.iter().any(|e| e.name == declared) {
            IrType::Int32
        } else {
            unreachable!("a verified program only names known types")
        };

        if !reference.nullable {
            return base;
        }
        IrType::Nullable(Nullable::of(base).expect("the checker rejects `Void?`"))
    }

    /// Lowers an expression whose value must survive lowering a later one.
    ///
    /// Values do not cross blocks in this IR (ADR-007), and `??`, `if`, `match`
    /// and the logical operators all open blocks of their own. When the later
    /// expression is one of those, the earlier value goes through a slot and is
    /// reloaded where it is used; when it is not, nothing is spilled and the
    /// IR stays as it was.
    fn lower_and_hold(&mut self, expr: &ast::Expr, will_branch: bool) -> Held {
        let value = self.lower_expr(expr);
        if !will_branch {
            return Held::Value(value);
        }

        let ty = self.type_of_operand(value);
        let slot = self.declare_slot("<held>", ty, expr.span());
        self.emit_effect(InstKind::Store(slot, value), expr.span());
        Held::Spilled(slot, ty)
    }

    /// Lowers a list of arguments, holding each across the ones that follow.
    ///
    /// An argument that opens blocks would strand every value already computed,
    /// so the earlier ones go through slots and are reloaded once the last has
    /// been lowered.
    fn lower_held_args(&mut self, args: &[ast::Arg], expected: &[IrType]) -> Vec<Operand> {
        let mut held = Vec::with_capacity(args.len());

        for (position, arg) in args.iter().enumerate() {
            let ty = expected.get(position).copied().unwrap_or(IrType::Int32);
            let branches_later = args[position + 1..]
                .iter()
                .any(|later| opens_blocks(&later.value));
            held.push(self.lower_and_hold_as(&arg.value, ty, branches_later));
        }

        held.into_iter()
            .zip(args)
            .map(|(value, arg)| self.reload(value, arg.span))
            .collect()
    }

    /// [`Self::lower_and_hold`], lowering into an expected type.
    fn lower_and_hold_as(&mut self, expr: &ast::Expr, ty: IrType, will_branch: bool) -> Held {
        let value = self.lower_expr_as(expr, ty);
        if !will_branch {
            return Held::Value(value);
        }

        let slot = self.declare_slot("<arg>", ty, expr.span());
        self.emit_effect(InstKind::Store(slot, value), expr.span());
        Held::Spilled(slot, ty)
    }

    /// Recovers a held value in whatever block is current now.
    fn reload(&mut self, held: Held, span: Span) -> Operand {
        match held {
            Held::Value(operand) => operand,
            Held::Spilled(slot, ty) => self.emit(InstKind::Load(slot), ty, span),
        }
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

        // The parameter types come from the checker, not from what was
        // written: `name?: T` is declared as `T` and resolved as `T?`, and the
        // slot has to hold the resolved one.
        let resolved: Vec<IrType> = self
            .checked
            .functions
            .get(&function.name.name)
            .map(|s| s.params.iter().map(|p| ir_type(p.ty)).collect())
            .expect("the checker records every declared function");

        let params: Vec<SlotId> = function
            .params
            .iter()
            .zip(resolved)
            .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span))
            .collect();

        self.lower_block(&function.body);

        // A `Void` function may end without an explicit return. In a non-`Void`
        // one the checker already proved every path returns, so an open block
        // here is one nothing reaches — the block after `loop { return x; }`
        // being the ordinary case.
        if self.return_type == IrType::Void {
            self.terminate(Terminator::Return(None));
        } else {
            self.terminate(Terminator::Unreachable);
        }

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

    /// Lowers one `construct` into a function over the object being built.
    ///
    /// `this` is an ordinary slot holding the object's address, which is what
    /// makes `this.name = value` a field write like any other rather than a
    /// form of its own.
    fn run_constructor(
        mut self,
        class: &ast::ClassDecl,
        constructor: &ast::ConstructDecl,
        id: u32,
        index: usize,
    ) -> (Function, Vec<Function>) {
        self.return_type = IrType::Void;

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let this = self.declare_slot("this", IrType::Object(id), class.name.span);
        self.initialize_defaults(this, id, class.name.span);

        let resolved: Vec<IrType> = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.constructors.get(index))
            .map(|params| params.iter().map(|p| ir_type(p.ty)).collect())
            .expect("the checker records every constructor");

        let mut params = vec![this];
        params.extend(
            constructor
                .params
                .iter()
                .zip(resolved)
                .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span)),
        );

        self.lower_block(&constructor.body);
        self.terminate(Terminator::Return(None));
        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            name: constructor_symbol(&class.name.name, index),
            params,
            return_type: IrType::Void,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: constructor.span,
        };

        (lowered, lifted)
    }

    /// Lowers a trait's default body into a function over the receiver.
    ///
    /// Its `this` is typed by the contract rather than by any class: the body
    /// was written knowing only what the contract declares, which is exactly
    /// what it may reach.
    fn run_contract_default(
        mut self,
        contract: &ast::ContractDecl,
        method: &ast::MethodDecl,
        body: &ast::Block,
        id: u32,
    ) -> (Function, Vec<Function>) {
        self.return_type = self.ir_type_from_ref(&method.return_type);

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let this = self.declare_slot("this", IrType::Contract(id), contract.name.span);

        let resolved: Vec<IrType> = self
            .checked
            .contracts
            .get(id as usize)
            .and_then(|c| c.method(&method.name.name))
            .map(|m| m.params.iter().map(|p| ir_type(p.ty)).collect())
            .expect("the checker records every contract method");

        let mut params = vec![this];
        params.extend(
            method
                .params
                .iter()
                .zip(resolved)
                .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span)),
        );

        self.lower_block(body);
        if self.return_type == IrType::Void {
            self.terminate(Terminator::Return(None));
        } else {
            self.terminate(Terminator::Unreachable);
        }
        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            name: contract_method_symbol(&contract.name.name, &method.name.name),
            params,
            return_type: self.return_type,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: method.span,
        };

        (lowered, lifted)
    }

    /// Writes every attribute's type default into a fresh object.
    ///
    /// `ZIRK_LANGUAGE_SPEC.md` section 7: an omitted attribute receives its
    /// default *before* any explicit initializer or the constructor runs.
    ///
    /// Zeroed memory is not a substitute. It happens to spell `0` and `false`,
    /// but a zeroed `String` is a null handle, not the empty string — and the
    /// difference shows the moment anyone compares it to `""`.
    fn initialize_defaults(&mut self, this: SlotId, id: u32, span: Span) {
        let fields: Vec<(u32, IrType)> = self.module.objects[id as usize]
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| (index as u32, field.ty))
            .collect();

        for (index, ty) in fields {
            let Some(value) = self.default_value(ty, span) else {
                // No default: the checker already required the constructor to
                // write it.
                continue;
            };
            let object = self.emit(InstKind::Load(this), IrType::Object(id), span);
            self.emit_effect(
                InstKind::StoreField {
                    object,
                    index,
                    value,
                },
                span,
            );
        }
    }

    /// The default value of a type, if it has one.
    fn default_value(&mut self, ty: IrType, span: Span) -> Option<Operand> {
        Some(match ty {
            IrType::Int32 => self.emit(InstKind::ConstInt(0), ty, span),
            IrType::Boolean => self.emit(InstKind::ConstBool(false), ty, span),
            IrType::String => {
                let id = self.module.intern_string("");
                self.emit(InstKind::ConstString(id), ty, span)
            }
            // Absence is exactly what a nullable type defaults to.
            IrType::Nullable(base) => self.emit(InstKind::NullValue(base), ty, span),
            IrType::Void | IrType::Closure(_) | IrType::Object(_) | IrType::Contract(_) => {
                return None;
            }
        })
    }

    /// Lowers one method into a function whose first parameter is the receiver.
    fn run_method(
        mut self,
        class: &ast::ClassDecl,
        method: &ast::MethodDecl,
        body: &ast::Block,
        id: u32,
        index: usize,
    ) -> (Function, Vec<Function>) {
        self.return_type = self.ir_type_from_ref(&method.return_type);

        let entry = self.new_block();
        self.current = entry;
        self.scopes.push(HashMap::new());

        let this = self.declare_slot("this", IrType::Object(id), class.name.span);

        let resolved: Vec<IrType> = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.methods.iter().find(|m| m.index == index))
            .map(|m| m.params.iter().map(|p| ir_type(p.ty)).collect())
            .expect("the checker records every method");

        let mut params = vec![this];
        params.extend(
            method
                .params
                .iter()
                .zip(resolved)
                .map(|(p, ty)| self.declare_slot(&p.name.name, ty, p.name.span)),
        );

        self.lower_block(body);

        // Same rule as an ordinary function: a `Void` one may end without an
        // explicit return, and in any other the checker already proved every
        // path returns.
        if self.return_type == IrType::Void {
            self.terminate(Terminator::Return(None));
        } else {
            self.terminate(Terminator::Unreachable);
        }

        self.scopes.pop();

        let lifted = std::mem::take(&mut self.lifted);
        let lowered = Function {
            name: method_symbol(&class.name.name, &method.name.name),
            params,
            return_type: self.return_type,
            slots: self.slots,
            blocks: self.blocks,
            entry,
            span: method.span,
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
        match &stmt.target {
            ast::AssignTarget::Name(name) => {
                let slot = self.lookup_slot(&name.name);
                let value = self.lower_expr_as(&stmt.value, self.slot_type(slot));
                self.emit_effect(InstKind::Store(slot, value), stmt.span);
            }
            ast::AssignTarget::Field(field) => {
                // The object is evaluated before the value, which is the order
                // it is written in.
                let object = self.lower_expr(&field.object);
                let (index, ty) = self.field_position(&field.object, &field.name.name);
                let value = self.lower_expr_as(&stmt.value, ty);
                self.emit_effect(
                    InstKind::StoreField {
                        object,
                        index,
                        value,
                    },
                    stmt.span,
                );
            }
        }
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

        // `do ... while` differs from `while` in exactly this edge: execution
        // enters the body instead of the header, so the condition is first
        // checked after one run. Everything below is shared.
        if stmt.kind.body_runs_first() {
            self.terminate(Terminator::Jump(body_block));
        } else {
            self.terminate(Terminator::Jump(header));
        }

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

            // `&&` and `||` must not evaluate their right operand when the
            // left already decides the answer.
            ast::Expr::Binary(e) if matches!(e.op, ast::BinaryOp::And | ast::BinaryOp::Or) => {
                self.lower_short_circuit(e, span)
            }

            ast::Expr::Binary(e) => {
                let held = self.lower_and_hold(&e.left, opens_blocks(&e.right));
                let right = self.lower_expr(&e.right);
                let left = self.reload(held, e.left.span());
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
                let IrType::Closure(id) = self.type_of(&e.callee, e.callee.span()) else {
                    unreachable!("checked by `is_closure_call`")
                };
                let branching = e.args.iter().any(|a| opens_blocks(&a.value));
                let held = self.lower_and_hold(&e.callee, branching);

                let expected = self.module.closures[id as usize].params.clone();
                let returns = self.module.closures[id as usize].returns;
                let args = self.lower_held_args(&e.args, &expected);
                let callee = self.reload(held, e.callee.span());
                self.emit(InstKind::CallClosure { id, callee, args }, returns, span)
            }

            ast::Expr::Call(e) => {
                if let Some(operand) = self.lower_super_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_contract_call(e, span) {
                    return operand;
                }
                if let Some(operand) = self.lower_method_call(e, span) {
                    return operand;
                }
                let name = self.callee_name(e);
                if let Some(id) = self.class_id(&name) {
                    return self.lower_construction(e, id, span);
                }
                let args = self.lower_args(e);
                let returns = self.signature_return(&name);
                self.emit(InstKind::Call { callee: name, args }, returns, span)
            }

            ast::Expr::If(e) => self.lower_if_expr(e, span),
            ast::Expr::Field(e) => self.lower_field(e, span),
            ast::Expr::This(_) | ast::Expr::Super(_) => {
                // `super` names where to look, not what to look at: the value
                // is the same instance.
                let slot = self.lookup_slot("this");
                self.emit(InstKind::Load(slot), self.slot_type(slot), span)
            }
            ast::Expr::Ternary(e) => self.lower_ternary(e, span),
            ast::Expr::Increment(e) => self.lower_increment(e, span),
            ast::Expr::Match(e) => self.lower_match(e, span),
            ast::Expr::Variant(e) => {
                let value = self.discriminant(&e.enum_name, &e.variant.name);
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

    /// Lowers `&&` and `||` so the right operand only runs when it matters.
    ///
    /// ```text
    ///   a ──(decides)──▶ result = <that answer>  ──┐
    ///   │(undecided)                               │
    ///   ▼                                          ▼
    ///   result = b ──────────────────────────▶ continue
    /// ```
    ///
    /// `false && f()` and `true || f()` must not call `f`. Evaluating both
    /// operands was the Phase 1 behaviour, noted there as pending precisely
    /// because short-circuiting needs blocks of its own.
    fn lower_short_circuit(&mut self, expr: &ast::BinaryExpr, span: Span) -> Operand {
        let is_and = expr.op == ast::BinaryOp::And;
        let result = self.declare_slot("<logic>", IrType::Boolean, span);

        let left = self.lower_expr(&expr.left);

        let rest_block = self.new_block();
        let decided_block = self.new_block();
        let continue_block = self.new_block();

        // `&&` continues when the left is true; `||` when it is false.
        let (then_block, else_block) = if is_and {
            (rest_block, decided_block)
        } else {
            (decided_block, rest_block)
        };

        self.terminate(Terminator::Branch {
            condition: left,
            then_block,
            else_block,
        });

        // The left operand already decided: `false` for `&&`, `true` for `||`.
        self.current = decided_block;
        let decided = self.emit(InstKind::ConstBool(!is_and), IrType::Boolean, span);
        self.emit_effect(InstKind::Store(result, decided), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = rest_block;
        let right = self.lower_expr(&expr.right);
        self.emit_effect(InstKind::Store(result, right), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), IrType::Boolean, span)
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
        let inner = self.emit(
            InstKind::Unwrap(held),
            self.slot_type(holder).unwrapped(),
            span,
        );
        // The result may still be nullable when the fallback is: widening the
        // unwrapped value back keeps both branches storing the same type.
        let inner = match result_type {
            IrType::Nullable(base) => {
                self.emit(InstKind::Wrap { base, value: inner }, result_type, span)
            }
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

        // The types come from the slots the captures live in, not from the
        // checker's types. A captured closure is the reason: the checker
        // identifies a closure type by its own numbering and the IR by the
        // layout it built, and only the slot knows which layout this one is.
        let capture_slots: Vec<SlotId> = info
            .captures
            .iter()
            .map(|c| self.lookup_slot(&c.name))
            .collect();
        let capture_types: Vec<IrType> = capture_slots
            .iter()
            .map(|slot| self.slot_type(*slot))
            .collect();
        let signature = &self.checked.fn_types[info.fn_type as usize];
        let param_types: Vec<IrType> = signature.params.iter().map(|t| ir_type(*t)).collect();
        let returns = ir_type(signature.returns);

        // The name reaches the object file as a symbol, so it may only use
        // characters every target's assembler accepts. `#` starts a comment in
        // x86_64's AT&T syntax — with `<lambda>#0` the symbol broke there and
        // nowhere else, since aarch64 comments with `//`.
        //
        // A dot is valid in ELF, Mach-O and COFF symbols, and impossible in a
        // Zirk identifier, so it cannot collide with a user's function.
        let name = format!("lambda.{}", self.module.closures.len());
        let id = self.module.closures.len() as u32;
        self.module.closures.push(ClosureLayout {
            function: name.clone(),
            captures: capture_types.clone(),
            params: param_types.clone(),
            returns,
        });

        // The captures are read in the enclosing function, where their slots
        // live, before the body is lifted out.
        let captures: Vec<Operand> = capture_slots
            .iter()
            .map(|slot| self.emit(InstKind::Load(*slot), self.slot_type(*slot), span))
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

        self.emit(
            InstKind::MakeClosure { id, captures },
            IrType::Closure(id),
            span,
        )
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
        let mut inner = FunctionLowering::new(self.module, self.checked, self.declarations);
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
    fn discriminant(&self, enum_name: &ast::Ident, variant: &str) -> i32 {
        let declared = self.declaration_of(&enum_name.name, enum_name.span);
        self.checked
            .enums
            .iter()
            .find(|e| e.name == declared)
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
        let result =
            (result_type != IrType::Void).then(|| self.declare_slot("<match>", result_type, span));

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
                let value = self.discriminant(&v.enum_name, &v.variant.name);
                self.emit(InstKind::ConstInt(value), IrType::Int32, span)
            }
            // `null` tests absence rather than a value, so it is the one
            // pattern that does not compare against anything.
            ast::Pattern::Null(_) => {
                let value = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
                return self.emit(InstKind::IsNull(value), IrType::Boolean, span);
            }
            ast::Pattern::Wildcard(_) | ast::Pattern::Binding(_) => {
                unreachable!("an irrefutable pattern is not tested this way")
            }
        };

        let left = self.emit(InstKind::Load(scrutinee), scrutinee_type, span);
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
        // The first arm decides: the checker already proved every arm agrees,
        // so looking further would only confirm what it verified.
        let Some(first) = expr.arms.first() else {
            return IrType::Void;
        };

        match &first.body {
            ast::ArmBody::Expr(e) => self.type_of(e, e.span()),
            ast::ArmBody::Block(b) if matches!(b.statements.last(), Some(ast::Stmt::Expr(_))) => {
                self.block_value_type(b)
            }
            // A block that does not end in an expression produces nothing, and
            // that makes the whole `match` a statement.
            ast::ArmBody::Block(_) => IrType::Void,
        }
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

    /// Lowers `super(...)` and `super.method(...)`.
    ///
    /// Both are direct calls: `super` names a body statically, which is the
    /// whole point of writing it instead of letting dispatch decide.
    fn lower_super_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let base = self.enclosing_base()?;

        // `super.method(...)`
        if let ast::Expr::Field(field) = &*call.callee
            && matches!(&*field.object, ast::Expr::Super(_))
        {
            let class = &self.checked.classes[base as usize];
            let method = class.method(&field.name.name)?;
            let name = body_symbol(self.checked, method);
            let returns = ir_type(method.returns);
            let params: Vec<IrType> = method.params.iter().map(|p| ir_type(p.ty)).collect();

            let this = self.lookup_slot("this");
            let receiver = self.emit(InstKind::Load(this), self.slot_type(this), span);
            let mut args = vec![receiver];
            for (arg, ty) in call.args.iter().zip(params) {
                args.push(self.lower_expr_as(&arg.value, ty));
            }
            return Some(self.emit(InstKind::Call { callee: name, args }, returns, span));
        }

        if !matches!(&*call.callee, ast::Expr::Super(_)) {
            return None;
        }

        // `super(...)`: the base's constructor, on this same instance.
        let index = self.constructor_index(base, call.args.len());
        let params: Vec<IrType> = self.checked.classes[base as usize].constructors[index]
            .iter()
            .map(|p| ir_type(p.ty))
            .collect();

        let this = self.lookup_slot("this");
        let receiver = self.emit(InstKind::Load(this), self.slot_type(this), span);
        let mut args = vec![receiver];
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        let name = self.checked.classes[base as usize].name.clone();
        self.emit_effect(
            InstKind::Call {
                callee: constructor_symbol(&name, index),
                args,
            },
            span,
        );

        // `super(...)` produces nothing: it is run for its effect on `this`.
        Some(self.emit(InstKind::ConstBool(false), IrType::Boolean, span))
    }

    /// The base of the class whose body is being lowered.
    fn enclosing_base(&self) -> Option<u32> {
        let slot = self.scopes.iter().rev().find_map(|s| s.get("this"))?;
        let IrType::Object(id) = self.slot_type(*slot) else {
            return None;
        };
        self.checked.classes.get(id as usize)?.base
    }

    /// The contract method a call invokes, if the receiver is a contract.
    fn contract_method_of(&self, call: &ast::CallExpr) -> Option<&zirk_sema::ContractMethod> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        let IrType::Contract(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };
        self.checked.contracts[id as usize].method(&field.name.name)
    }

    /// The method a call invokes, if it is a method call.
    fn method_of(&self, call: &ast::CallExpr) -> Option<&zirk_sema::MethodInfo> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        let IrType::Object(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };
        self.checked.classes[id as usize].method(&field.name.name)
    }

    /// Lowers `value.method(...)` where the receiver is reached by contract.
    fn lower_contract_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        let IrType::Contract(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };

        let contract = &self.checked.contracts[id as usize];
        let method = contract.method(&field.name.name)?;
        let index = method.index as u32;
        let returns = ir_type(method.returns);
        let params: Vec<IrType> = method.params.iter().map(|p| ir_type(p.ty)).collect();

        let receiver = self.lower_expr(&field.object);
        let mut args = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        Some(self.emit(
            InstKind::CallContract {
                object: receiver,
                contract: id,
                index,
                args,
            },
            returns,
            span,
        ))
    }

    /// Lowers `object.method(...)` into a direct call with the receiver first.
    ///
    /// Direct because no method is redefinable yet: without inheritance every
    /// call has exactly one target, and paying an indirection for a generality
    /// the program cannot use would be paying for nothing (D3).
    fn lower_method_call(&mut self, call: &ast::CallExpr, span: Span) -> Option<Operand> {
        let ast::Expr::Field(field) = &*call.callee else {
            return None;
        };
        if self.checked.variant_accesses.contains(&field.span) {
            return None;
        }
        let IrType::Object(id) = self.type_of(&field.object, field.object.span()) else {
            return None;
        };

        let class = &self.checked.classes[id as usize];
        let method = class.method(&field.name.name)?;
        // The body lives where it was declared, which is not always the class
        // being called through: an inherited method keeps its owner, and an
        // adopted trait default belongs to the trait.
        let name = body_symbol(self.checked, method);
        let returns = ir_type(method.returns);
        let params: Vec<IrType> = method.params.iter().map(|p| ir_type(p.ty)).collect();

        let virtual_index = method.overridden.then_some(method.index as u32);

        let receiver = self.lower_expr(&field.object);
        let mut args = Vec::new();
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        // A method some subclass redefines has no statically known target, so
        // it goes through the object's own table. Every other call is direct.
        Some(match virtual_index {
            Some(index) => self.emit(
                InstKind::CallVirtual {
                    object: receiver,
                    index,
                    args,
                },
                returns,
                span,
            ),
            None => {
                let mut all = vec![receiver];
                all.extend(args);
                self.emit(
                    InstKind::Call {
                        callee: name,
                        args: all,
                    },
                    returns,
                    span,
                )
            }
        })
    }

    /// The layout id of a class, if the name is one.
    fn class_id(&self, name: &str) -> Option<u32> {
        self.module
            .objects
            .iter()
            .position(|o| o.name == name)
            .map(|i| i as u32)
    }

    /// Lowers `User(...)` into an allocation plus a call to its constructor.
    ///
    /// The two steps are what separates identity from initialization: the
    /// object exists — and has its address, which is its identity — before its
    /// constructor runs on it.
    fn lower_construction(&mut self, call: &ast::CallExpr, id: u32, span: Span) -> Operand {
        let object = self.emit(InstKind::Alloc(id), IrType::Object(id), span);

        let index = self.constructor_index(id, call.args.len());
        let params: Vec<IrType> = self
            .checked
            .classes
            .get(id as usize)
            .and_then(|c| c.constructors.get(index))
            .map(|p| p.iter().map(|param| ir_type(param.ty)).collect())
            .expect("the checker resolved the constructor");

        let mut args = vec![object];
        for (arg, ty) in call.args.iter().zip(params) {
            args.push(self.lower_expr_as(&arg.value, ty));
        }

        let name = self.module.objects[id as usize].name.clone();
        self.emit_effect(
            InstKind::Call {
                callee: constructor_symbol(&name, index),
                args,
            },
            span,
        );

        object
    }

    /// Which `construct` a call of this arity resolves to.
    fn constructor_index(&self, id: u32, arity: usize) -> usize {
        self.checked.classes[id as usize]
            .constructors
            .iter()
            .position(|params| {
                let required = params
                    .iter()
                    .filter(|p| !p.optional && !p.variadic && !p.has_default)
                    .count();
                let variadic = params.iter().any(|p| p.variadic);
                arity >= required && (variadic || arity <= params.len())
            })
            .expect("the checker resolved the constructor")
    }

    /// Lowers `a.b`, which is a variant access or a field read.
    ///
    /// The checker already decided which one it is and recorded the answer, so
    /// this reads the decision rather than repeating it with the same tables.
    fn lower_field(&mut self, expr: &ast::FieldExpr, span: Span) -> Operand {
        if self.checked.variant_accesses.contains(&expr.span) {
            let ast::Expr::Path(enum_name) = &*expr.object else {
                unreachable!("a variant access names its enum")
            };
            let value = self.discriminant(enum_name, &expr.name.name);
            return self.emit(InstKind::ConstInt(value), IrType::Int32, span);
        }

        let object = self.lower_expr(&expr.object);
        let (index, ty) = self.field_position(&expr.object, &expr.name.name);
        self.emit(InstKind::LoadField { object, index }, ty, span)
    }

    /// The type a member access produces.
    fn field_type_of(&self, expr: &ast::FieldExpr) -> IrType {
        if self.checked.variant_accesses.contains(&expr.span) {
            // A variant without associated data is exactly its discriminant.
            return IrType::Int32;
        }
        self.field_position(&expr.object, &expr.name.name).1
    }

    /// Where a field sits in its object, and what type it holds.
    fn field_position(&self, object: &ast::Expr, name: &str) -> (u32, IrType) {
        let IrType::Object(id) = self.type_of(object, object.span()) else {
            unreachable!("a verified field access reads an object")
        };
        let layout = &self.module.objects[id as usize];
        let index = layout
            .field_index(name)
            .expect("the checker resolved the field against this layout");
        (index as u32, layout.fields[index].ty)
    }

    /// Lowers `cond ? a : b`.
    ///
    /// The same shape as the `if` expression — two blocks writing one slot —
    /// because it means the same thing. What it does not share is the branches
    /// being blocks: here they are expressions, so there is nothing to scope.
    fn lower_ternary(&mut self, expr: &ast::TernaryExpr, span: Span) -> Operand {
        let ty = self.type_of(&expr.when_true, expr.when_true.span());
        let result = self.declare_slot("<ternary>", ty, span);

        let condition = self.lower_expr(&expr.condition);
        let then_block = self.new_block();
        let else_block = self.new_block();
        let continue_block = self.new_block();

        self.terminate(Terminator::Branch {
            condition,
            then_block,
            else_block,
        });

        // Only the selected branch runs: the operand of the other one is never
        // evaluated, which is what makes a ternary usable as a guard.
        self.current = then_block;
        let value = self.lower_expr(&expr.when_true);
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = else_block;
        let value = self.lower_expr(&expr.when_false);
        self.emit_effect(InstKind::Store(result, value), span);
        self.terminate(Terminator::Jump(continue_block));

        self.current = continue_block;
        self.emit(InstKind::Load(result), ty, span)
    }

    /// Lowers `i++`, `++i`, `i--` and `--i` used as a value.
    ///
    /// Both forms perform the same update; they differ only in which value the
    /// expression yields, and that is decided by whether the previous value is
    /// read before or after the store.
    fn lower_increment(&mut self, expr: &ast::IncrementExpr, span: Span) -> Operand {
        let slot = self.lookup_slot(expr.target.name());
        let ty = self.slot_type(slot);

        let previous = self.emit(InstKind::Load(slot), ty, span);
        let one = self.emit(InstKind::ConstInt(1), IrType::Int32, span);
        let updated = self.emit(
            InstKind::Binary {
                op: binary_op(expr.op.as_binary()),
                left: previous,
                right: one,
            },
            ty,
            span,
        );
        self.emit_effect(InstKind::Store(slot, updated), span);

        match expr.fix {
            ast::IncrementFix::Prefix => updated,
            ast::IncrementFix::Postfix => previous,
        }
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

        let value = match last {
            ast::Stmt::Expr(e) => self.lower_expr(&e.expr),
            // Same reason as in the checker: an `if` is parsed as a statement
            // wherever it appears, and the position decides what it is.
            ast::Stmt::If(nested) => self.lower_if_expr(nested, nested.span),
            _ => unreachable!("a verified block used as a value ends in an expression"),
        };

        self.scopes.pop();
        value
    }

    /// The type a block used as a value produces.
    fn block_value_type(&self, block: &ast::Block) -> IrType {
        match block.statements.last() {
            Some(ast::Stmt::Expr(e)) => self.type_of(&e.expr, e.expr.span()),
            Some(ast::Stmt::If(nested)) => self.block_value_type(&nested.then_branch),
            _ => unreachable!("a verified block used as a value ends in an expression"),
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
            // `super(...)` and a method call know their own target, so they
            // need none of the name resolution the arm below does.
            ast::Expr::Call(e)
                if matches!(&*e.callee, ast::Expr::Super(_))
                    || matches!(&*e.callee, ast::Expr::Field(f) if matches!(&*f.object, ast::Expr::Super(_))) =>
            {
                self.lower_super_call(e, expr.span());
            }
            ast::Expr::Call(e) if self.contract_method_of(e).is_some() => {
                self.lower_contract_call(e, expr.span());
            }
            ast::Expr::Call(e) if self.method_of(e).is_some() => {
                self.lower_method_call(e, expr.span());
            }
            // A closure call goes through the value and has its own arm in
            // `lower_expr`; only a direct call is special-cased here.
            ast::Expr::Call(e)
                if !self.is_closure_call(e) && {
                    let name = self.callee_name(e);
                    self.signature_return(&name)
                } == IrType::Void =>
            {
                let name = self.callee_name(e).to_string();
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

    /// Lowers the arguments of a call into the parameters they fill.
    ///
    /// Named arguments are placed by name and omitted ones take their default,
    /// so the call reaching the IR always has the declared arity. That is
    /// decision D4: neither the IR nor LLVM ever sees a named or missing
    /// argument.
    fn lower_args(&mut self, call: &ast::CallExpr) -> Vec<Operand> {
        let name = self.callee_name(call);
        let signature = self
            .checked
            .functions
            .get(&name)
            .expect("a verified program only calls declared functions");
        let params: Vec<(String, IrType)> = signature
            .params
            .iter()
            .map(|p| (p.name.clone(), ir_type(p.ty)))
            .collect();

        let mut slots: Vec<Option<Held>> = (0..params.len()).map(|_| None).collect();
        let mut next = 0usize;

        for (position, arg) in call.args.iter().enumerate() {
            let index = match &arg.name {
                Some(named) => params
                    .iter()
                    .position(|(name, _)| *name == named.name)
                    .expect("the checker resolved every named argument"),
                None => {
                    while next < slots.len() && slots[next].is_some() {
                        next += 1;
                    }
                    let index = next;
                    next += 1;
                    index
                }
            };
            // A later argument that opens blocks would strand the ones already
            // computed, so each is held until every argument is lowered.
            let branches_later = call.args[position + 1..]
                .iter()
                .any(|a| opens_blocks(&a.value));
            slots[index] =
                Some(self.lower_and_hold_as(&arg.value, params[index].1, branches_later));
        }

        let declaration = self.declarations.get(name.as_str()).copied();

        slots
            .into_iter()
            .enumerate()
            .map(|(index, filled)| match filled {
                Some(held) => self.reload(held, call.span),
                None => self.lower_default(declaration, index, params[index].1, call.span),
            })
            .collect()
    }

    /// The value a parameter takes when the call omits it.
    fn lower_default(
        &mut self,
        declaration: Option<&ast::FnDecl>,
        index: usize,
        ty: IrType,
        span: Span,
    ) -> Operand {
        let param = declaration
            .and_then(|d| d.params.get(index))
            .expect("the checker rejects a call missing a parameter with no default");

        match &param.default {
            Some(expr) => self.lower_expr_as(expr, ty),
            // An optional parameter with no default is absent, and absence is
            // `null`: the checker made its type nullable for exactly this.
            None => {
                let IrType::Nullable(base) = ty else {
                    unreachable!("an omitted parameter without a default is nullable")
                };
                self.emit(InstKind::NullValue(base), ty, span)
            }
        }
    }

    /// Whether a call goes through a closure value rather than a name.
    fn is_closure_call(&self, call: &ast::CallExpr) -> bool {
        // A method or `super` call is not a closure call, and asking for the
        // type of its callee would ask for the type of a method — which is not
        // a value.
        if self.method_of(call).is_some()
            || self.contract_method_of(call).is_some()
            || matches!(&*call.callee, ast::Expr::Super(_))
        {
            return false;
        }

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
            ast::Expr::Call(e) if matches!(&*e.callee, ast::Expr::Super(_)) => IrType::Void,
            ast::Expr::Call(e) => {
                if let Some(method) = self.contract_method_of(e) {
                    return ir_type(method.returns);
                }
                if let Some(method) = self.method_of(e) {
                    return ir_type(method.returns);
                }
                let name = self.callee_name(e);
                match self.class_id(&name) {
                    Some(id) => IrType::Object(id),
                    None => self.signature_return(&name),
                }
            }
            ast::Expr::If(e) => self.block_value_type(&e.then_branch),
            ast::Expr::Field(e) => self.field_type_of(e),
            ast::Expr::This(_) | ast::Expr::Super(_) => self.slot_type(self.lookup_slot("this")),
            ast::Expr::Ternary(e) => self.type_of(&e.when_true, e.when_true.span()),
            // Both forms yield the type of the operand they update.
            ast::Expr::Increment(e) => self.slot_type(self.lookup_slot(e.target.name())),
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

/// Where a value waits while a later expression is lowered.
enum Held {
    /// Still in its block, because nothing moved the insertion point.
    Value(Operand),
    /// Parked in a slot, because the later expression opened blocks.
    Spilled(SlotId, IrType),
}

/// Whether lowering this expression opens blocks of its own.
///
/// Only these constructs move the insertion point, and that is what forces an
/// earlier value to travel through a slot.
fn opens_blocks(expr: &ast::Expr) -> bool {
    match expr {
        // A ternary opens blocks for the same reason an `if` does: only one of
        // its branches runs.
        ast::Expr::If(_) | ast::Expr::Match(_) | ast::Expr::Ternary(_) => true,
        ast::Expr::Binary(e) => {
            matches!(
                e.op,
                ast::BinaryOp::Coalesce | ast::BinaryOp::And | ast::BinaryOp::Or
            ) || opens_blocks(&e.left)
                || opens_blocks(&e.right)
        }
        ast::Expr::Unary(e) => opens_blocks(&e.operand),
        ast::Expr::Call(e) => e.args.iter().any(|a| opens_blocks(&a.value)),
        ast::Expr::Println(e) => opens_blocks(&e.arg),
        _ => false,
    }
}

/// The name of a directly called function.
///
/// Calling a closure value goes through a different instruction, and the
/// checker rejects anything else, so a verified program only reaches here with
/// a plain name.
fn callee_ident(call: &ast::CallExpr) -> &ast::Ident {
    match &*call.callee {
        ast::Expr::Path(ident) => ident,
        _ => unreachable!("a verified program calls a name or a closure value"),
    }
}
