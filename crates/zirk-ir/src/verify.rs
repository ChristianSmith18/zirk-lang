//! Well-formedness verifier for the IR.
//!
//! It exists so a lowering bug surfaces as a precise message here instead of as
//! an unreadable LLVM failure — or, worse, as a binary that miscompiles
//! silently. It is the IR equivalent of `Module::verify()` in LLVM.
//!
//! It is not a type checker for Zirk: by the time we reach the IR the program
//! has already been verified by `zirk-sema`. What is checked here are the
//! invariants of the *representation*.

use crate::ir::*;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// A violation of the IR invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrError {
    pub function: String,
    pub detail: String,
}

impl std::fmt::Display for IrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "in `{}`: {}", self.function, self.detail)
    }
}

/// Verifies an entire module.
pub fn verify(module: &Module) -> Result<(), Vec<IrError>> {
    let mut errors = Vec::new();

    verify_module_layouts(module, &mut errors);
    verify_mutex_callback_layouts(module, &mut errors);

    // Function verification calls helpers such as `is_managed_reference`
    // that index validated module layout tables. Stop before those accesses
    // when the module-level representation is malformed.
    if errors.is_empty() {
        let direct_effects = direct_call_effects(module);
        let mutex_callback_functions = mutex_callback_functions(module);
        for function in &module.functions {
            verify_function(
                module,
                function,
                &direct_effects,
                &mutex_callback_functions,
                &mut errors,
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn module_error(errors: &mut Vec<IrError>, detail: impl Into<String>) {
    errors.push(IrError {
        function: "<module>".into(),
        detail: detail.into(),
    });
}

/// Resolves a non-empty path through inline Value layouts. Enum traversal is
/// deliberately unavailable until IR carries an explicit active-variant guard.
fn resolve_borrowed_value_path(module: &Module, root: IrType, path: &[u32]) -> Option<IrType> {
    if path.is_empty() {
        return None;
    }
    let mut ty = root;
    for &index in path {
        let IrType::Value(layout) = ty else {
            return None;
        };
        ty = module
            .values
            .get(layout as usize)?
            .fields
            .get(index as usize)?
            .ty;
    }
    Some(ty)
}

fn verify_mutex_callback_layouts(module: &Module, errors: &mut Vec<IrError>) {
    let used: HashSet<u32> = module
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.instructions.iter())
        .filter_map(|i| {
            if let InstKind::MutexWithCall { layout, .. } = &i.kind {
                Some(*layout)
            } else {
                None
            }
        })
        .collect();
    for (index, layout) in module.closures.iter().enumerate() {
        let has_view = layout
            .params
            .iter()
            .any(|ty| matches!(ty, IrType::BorrowedView { .. }))
            || layout
                .captures
                .iter()
                .any(|ty| matches!(ty, IrType::BorrowedView { .. }))
            || matches!(layout.returns, IrType::BorrowedView { .. });
        if has_view && !used.contains(&(index as u32)) {
            module_error(
                errors,
                format!(
                    "closure layout {index} contains BorrowedView but is not used by MutexWithCall"
                ),
            );
        }
        if used.contains(&(index as u32)) {
            if layout.params.len() != 1
                || !matches!(layout.params.first(), Some(IrType::BorrowedView { .. }))
            {
                module_error(
                    errors,
                    format!(
                        "MutexWithCall closure layout {index} must have exactly one BorrowedView parameter"
                    ),
                );
            }
            if layout
                .captures
                .iter()
                .any(|ty| matches!(ty, IrType::BorrowedView { .. }))
                || matches!(layout.returns, IrType::BorrowedView { .. })
            {
                module_error(
                    errors,
                    format!(
                        "MutexWithCall closure layout {index} cannot capture or return BorrowedView"
                    ),
                );
            }
        }
    }
}

fn mutex_callback_functions(module: &Module) -> HashSet<String> {
    let used_layouts: HashSet<u32> = module
        .functions
        .iter()
        .flat_map(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .filter_map(|inst| {
                    if let InstKind::MutexWithCall { layout, .. } = &inst.kind {
                        Some(*layout)
                    } else {
                        None
                    }
                })
        })
        .collect();
    module
        .functions
        .iter()
        .flat_map(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| block.instructions.iter())
                .filter_map(|inst| {
                    let InstKind::MakeCallable { target, .. } = &inst.kind else {
                        return None;
                    };
                    let IrType::Callable(layout) = inst.ty else {
                        return None;
                    };
                    used_layouts.contains(&layout).then(|| target.clone())
                })
        })
        .collect()
}

/// Whether an instruction may cross a scheduler, host-blocking, shared-runtime,
/// or GC/safepoint boundary while a BorrowedView is live. Direct calls to
/// module functions are handled by `direct_call_effects` below.
fn has_intrinsic_borrow_boundary(kind: &InstKind) -> bool {
    matches!(
        kind,
        InstKind::CallContract { .. }
            | InstKind::CallVirtual { .. }
            | InstKind::CallClosure { .. }
            | InstKind::CallCallable { .. }
            | InstKind::BranchStart { .. }
            | InstKind::ParallelForStart { .. }
            | InstKind::BackgroundStart { .. }
            | InstKind::ServiceStart { .. }
            | InstKind::ServiceReady { .. }
            | InstKind::MutexAcquire { .. }
            | InstKind::MutexWithCall { .. }
            | InstKind::MutexNew { .. }
            | InstKind::ProjectGlobalInit { .. }
            | InstKind::ProjectGlobalGet { .. }
            | InstKind::ProjectGlobalSet { .. }
            | InstKind::ProjectGlobalWatchSubscribe { .. }
            | InstKind::ProjectGlobalWatchNext { .. }
            | InstKind::ProjectGlobalWatchClose { .. }
            | InstKind::ThreadRun { .. }
            | InstKind::SafepointPoll
            | InstKind::Alloc(_)
            | InstKind::ObjectNew
            | InstKind::AtomicNew { .. }
            | InstKind::MakeClosure { .. }
            | InstKind::MakeCallable { .. }
            | InstKind::Concat { .. }
            | InstKind::Repeat { .. }
            | InstKind::ToString(_)
            | InstKind::Float128Format { .. }
            | InstKind::Println(_)
            | InstKind::Print(_)
            | InstKind::PrintlnStderr(_)
            | InstKind::PrintStderr(_)
            | InstKind::FlushStdout { .. }
            | InstKind::FlushStderr { .. }
            | InstKind::StdinReadLineResult { .. }
            | InstKind::StdinReadChar { .. }
            | InstKind::StdinReadBytes { .. }
            | InstKind::StdinReadExact { .. }
            | InstKind::EnvironmentGet { .. }
            | InstKind::EnvironmentContains { .. }
            | InstKind::EnvironmentListNames { .. }
            | InstKind::FilesystemCall { .. }
            | InstKind::NetworkTcpConnect { .. }
            | InstKind::NetworkDnsResolve { .. }
            | InstKind::NetworkInterfacesSnapshot { .. }
            | InstKind::NetworkIpAddressParse { .. }
            | InstKind::NetworkTcpOriginNormalize { .. }
            | InstKind::NetworkTlsConnect { .. }
            | InstKind::NetworkTlsConnectWithRoots { .. }
            | InstKind::NetworkTlsConnectWithAlpn { .. }
            | InstKind::NetworkTlsConnectWithClientIdentity { .. }
            | InstKind::NetworkTlsUpgradeTcp { .. }
            | InstKind::NetworkTlsListen { .. }
            | InstKind::NetworkTlsListenWithClientAuth { .. }
            | InstKind::NetworkTlsAccept { .. }
            | InstKind::NetworkTlsCloseListener { .. }
            | InstKind::NetworkTlsListenerIsClosed { .. }
            | InstKind::NetworkTlsPeerAddress { .. }
            | InstKind::NetworkTlsNegotiatedAlpn { .. }
            | InstKind::NetworkTlsPeerCertificateSha256 { .. }
            | InstKind::NetworkTlsRead { .. }
            | InstKind::NetworkTlsWriteAll { .. }
            | InstKind::NetworkTlsShutdown { .. }
            | InstKind::NetworkTlsClose { .. }
            | InstKind::NetworkTlsIsClosed { .. }
            | InstKind::NetworkTcpRead { .. }
            | InstKind::NetworkTcpWrite { .. }
            | InstKind::NetworkTcpWriteAll { .. }
            | InstKind::NetworkTcpShutdown { .. }
            | InstKind::NetworkTcpClose { .. }
            | InstKind::NetworkTcpIsClosed { .. }
            | InstKind::NetworkTcpListen { .. }
            | InstKind::NetworkTcpAccept { .. }
            | InstKind::NetworkTcpCloseListener { .. }
            | InstKind::NetworkTcpListenerIsClosed { .. }
            | InstKind::NetworkUdpConnect { .. }
            | InstKind::NetworkUdpBind { .. }
            | InstKind::NetworkUdpSend { .. }
            | InstKind::NetworkUdpSendTo { .. }
            | InstKind::NetworkUdpSendBroadcast { .. }
            | InstKind::NetworkUdpSendMulticast { .. }
            | InstKind::NetworkUdpJoinMulticast { .. }
            | InstKind::NetworkUdpLeaveMulticast { .. }
            | InstKind::NetworkUdpReceive { .. }
            | InstKind::NetworkUdpClose { .. }
            | InstKind::NetworkUdpLocalAddress { .. }
            | InstKind::NetworkUdpIsClosed { .. }
            | InstKind::NetworkLocalConnect { .. }
            | InstKind::NetworkLocalBind { .. }
            | InstKind::NetworkLocalAccept { .. }
            | InstKind::NetworkLocalRead { .. }
            | InstKind::NetworkLocalWrite { .. }
            | InstKind::NetworkLocalShutdown { .. }
            | InstKind::NetworkLocalClose { .. }
            | InstKind::NetworkLocalIsClosed { .. }
            | InstKind::HttpGet { .. }
            | InstKind::HttpRequest { .. }
            | InstKind::HttpClientCreate { .. }
            | InstKind::HttpClientCreateWithOptions { .. }
            | InstKind::HttpClientCreateWithTrustRoots { .. }
            | InstKind::HttpClientCreateWithProxy { .. }
            | InstKind::HttpClientCreateWithProxyAndTrustRoots { .. }
            | InstKind::HttpClientRequest { .. }
            | InstKind::HttpClientRequestStream { .. }
            | InstKind::HttpClientAction { .. }
            | InstKind::HttpWebSocketConnect { .. }
            | InstKind::HttpWebSocketConnectWithTrustRoots { .. }
            | InstKind::HttpWebSocketSend { .. }
            | InstKind::HttpWebSocketReceive { .. }
            | InstKind::HttpWebSocketClose { .. }
            | InstKind::HttpWebSocketCloseWith { .. }
            | InstKind::HttpRequestSecret { .. }
            | InstKind::HttpHead { .. }
            | InstKind::HttpQuery { .. }
            | InstKind::HttpQueryWithContentType { .. }
            | InstKind::HttpPost { .. }
            | InstKind::HttpPostJson { .. }
            | InstKind::HttpPostBytes { .. }
            | InstKind::HttpGetStream { .. }
            | InstKind::HttpResponseToStream { .. }
            | InstKind::HttpRequestStream { .. }
            | InstKind::HttpRegisterServerResponseProducer { .. }
            | InstKind::HttpResponseReadChunk { .. }
            | InstKind::HttpResponseReadAll { .. }
            | InstKind::HttpResponseReadSseEvent { .. }
            | InstKind::HttpResponseSseHeartbeatCount { .. }
            | InstKind::HttpResponseStreamClose { .. }
            | InstKind::HttpUrlNormalize { .. }
            | InstKind::HttpUrlResolve { .. }
            | InstKind::HttpResponseText { .. }
            | InstKind::HttpResponseHeaderValues { .. }
            | InstKind::HttpFormParse { .. }
            | InstKind::HttpMultipartParse { .. }
            | InstKind::HttpUploadRead { .. }
            | InstKind::HttpUploadClose { .. }
            | InstKind::HttpServerControlCreate
            | InstKind::HttpServerControlRequestShutdown { .. }
            | InstKind::HttpRouteMatch { .. }
            | InstKind::HttpServerServeOnce { .. }
            | InstKind::HttpServerServeHttp3 { .. }
            | InstKind::HttpServerWebSocketServeOnce { .. }
            | InstKind::HttpServerWebSocketServeHttp3Once { .. }
            | InstKind::ScopeExit { .. }
            | InstKind::ScopeExitCollectAll { .. }
            | InstKind::JobWait { .. }
            | InstKind::JobWaitAny { .. }
            | InstKind::JobWaitAnyWithTimer { .. }
            | InstKind::JobOutcomeWaitTake { .. }
            | InstKind::ScopeOutcomeWaitTake { .. }
            | InstKind::TimerSleep { .. }
            | InstKind::ChannelSend { .. }
            | InstKind::ChannelRecv { .. }
            | InstKind::ChannelTrySend { .. }
            | InstKind::ChannelTryRecv { .. }
            | InstKind::ChannelNew { .. }
            | InstKind::ChannelClose { .. }
            | InstKind::ChannelLength(_)
            | InstKind::ChannelIsClosed(_)
            | InstKind::ChannelCapacity(_)
            | InstKind::ChannelDrain { .. }
            | InstKind::WeakFrom(_)
            | InstKind::WeakUpgrade(_)
            | InstKind::Clone(_)
            | InstKind::ArrayNew { .. }
            | InstKind::ListNew { .. }
            | InstKind::DequeNew { .. }
            | InstKind::DequePushFront { .. }
            | InstKind::DequePushBack { .. }
            | InstKind::DequePopFront(_)
            | InstKind::DequePopBack(_)
            | InstKind::DequeClear(_)
            | InstKind::MapNew { .. }
            | InstKind::SetNew { .. }
            | InstKind::MapKeys(_)
            | InstKind::MapSortedKeys(_)
            | InstKind::MapValues(_)
            | InstKind::MapEntries(_)
            | InstKind::ListAdd { .. }
            | InstKind::ListInsert { .. }
            | InstKind::ListRemove { .. }
            | InstKind::ListRemoveValue { .. }
            | InstKind::ArrayClone { .. }
            | InstKind::ListClone { .. }
            | InstKind::MapSet { .. }
            | InstKind::MapRemove { .. }
            | InstKind::MapClear(_)
            | InstKind::SetValues(_)
            | InstKind::SetSortedElements(_)
            | InstKind::SetRemove { .. }
            | InstKind::SetAdd { .. }
            | InstKind::SetClear(_)
            | InstKind::SetOperation { .. }
            | InstKind::ArraySlice { .. }
    )
}

/// Computes conservative whole-module effects for direct calls. Unknown calls
/// are effectful. Repeated propagation reaches the least fixed point, even for
/// recursive call graphs.
fn direct_call_effects(module: &Module) -> HashMap<String, bool> {
    let function_names: HashSet<&str> = module
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    let mut effects = HashMap::with_capacity(module.functions.len());
    let mut callers: HashMap<String, Vec<String>> = HashMap::new();

    for function in &module.functions {
        let mut effectful = false;
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.kind {
                InstKind::Call { callee, .. } if function_names.contains(callee.as_str()) => {
                    callers
                        .entry(callee.clone())
                        .or_default()
                        .push(function.name.clone());
                }
                InstKind::Call { callee, .. } => {
                    effectful |= !is_non_safepoint_runtime_leaf(callee)
                }
                kind => effectful |= has_intrinsic_borrow_boundary(kind),
            }
        }
        effects
            .entry(function.name.clone())
            .and_modify(|known| *known |= effectful)
            .or_insert(effectful);
    }

    // Propagate effectfulness from callees to callers once per call edge. The
    // queue also handles recursion without repeated whole-module rescans.
    let mut queue: VecDeque<String> = effects
        .iter()
        .filter_map(|(name, effectful)| effectful.then_some(name.clone()))
        .collect();
    while let Some(callee) = queue.pop_front() {
        for caller in callers.get(&callee).into_iter().flatten() {
            if let Some(effectful) = effects.get_mut(caller)
                && !*effectful
            {
                *effectful = true;
                queue.push_back(caller.clone());
            }
        }
    }
    effects
}

fn may_suspend_or_spawn(kind: &InstKind, direct_effects: &HashMap<String, bool>) -> bool {
    match kind {
        InstKind::Call { callee, .. } => direct_effects
            .get(callee)
            .copied()
            .unwrap_or_else(|| !is_non_safepoint_runtime_leaf(callee)),
        kind => has_intrinsic_borrow_boundary(kind),
    }
}

/// Runtime leaf calls proven not to allocate, suspend, block, or re-enter the
/// executor. Keep this allowlist narrow: every entry is used while a
/// `BorrowedView` may be live, so adding a symbol requires auditing its runtime
/// implementation and ABI contract.
fn is_non_safepoint_runtime_leaf(callee: &str) -> bool {
    non_safepoint_runtime_leaf_signature(callee).is_some()
}

fn non_safepoint_runtime_leaf_signature(callee: &str) -> Option<(IrType, IrType)> {
    use IntWidth::{I32, I64, I128};
    let (parameter, result) = match callee {
        "zirk_str_length" | "zirk_str_byte_length" => (IrType::String, IrType::Int(I32)),
        "zirk_str_is_empty" => (IrType::String, IrType::Boolean),
        "zirk_char_byte_length" | "zirk_char_codepoint_count" | "zirk_char_ascii_code" => {
            (IrType::Char, IrType::Int(I32))
        }
        "zirk_char_is_ascii"
        | "zirk_char_is_alphabetic"
        | "zirk_char_is_numeric"
        | "zirk_char_is_alphanumeric"
        | "zirk_char_is_uppercase"
        | "zirk_char_is_lowercase"
        | "zirk_char_is_digit"
        | "zirk_char_is_letter"
        | "zirk_char_is_whitespace" => (IrType::Char, IrType::Boolean),
        "zirk_duration_days" => (IrType::Int(I64), IrType::Int(I64)),
        "zirk_duration_hours"
        | "zirk_duration_minutes"
        | "zirk_duration_seconds"
        | "zirk_duration_milliseconds"
        | "zirk_duration_microseconds"
        | "zirk_duration_nanoseconds" => (IrType::Int(I64), IrType::Int(I32)),
        "zirk_range_start" | "zirk_range_end" | "zirk_range_step" => {
            (IrType::Range, IrType::Int(I64))
        }
        "zirk_rt_date_year"
        | "zirk_rt_date_month"
        | "zirk_rt_date_day"
        | "zirk_rt_date_day_of_week"
        | "zirk_rt_date_day_of_year"
        | "zirk_rt_date_week_of_year"
        | "zirk_rt_date_quarter"
        | "zirk_rt_date_days_in_month"
        | "zirk_rt_date_days_in_year" => (IrType::Int(I64), IrType::Int(I32)),
        "zirk_rt_date_is_leap" => (IrType::Int(I64), IrType::Boolean),
        "zirk_rt_time_hour" | "zirk_rt_time_minute" | "zirk_rt_time_second" => {
            (IrType::Int(I64), IrType::Int(I32))
        }
        "zirk_rt_time_nanosecond" => (IrType::Int(I64), IrType::Int(I64)),
        "zirk_rt_datetime_days" | "zirk_rt_datetime_nanos" => (IrType::Int(I128), IrType::Int(I64)),
        _ => return None,
    };
    Some((parameter, result))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum InlineLayout {
    Value(u32),
    Enum(u32),
    Closure(u32),
}

/// Inline aggregates must be finite. Opaque references such as Object, Array,
/// and Callable do not recurse through their pointee here.
fn has_inline_layout_cycle(
    module: &Module,
    ty: IrType,
    visiting: &mut HashSet<InlineLayout>,
) -> bool {
    match ty {
        IrType::Value(id) => {
            let key = InlineLayout::Value(id);
            if !visiting.insert(key) {
                return true;
            }
            let cycle = module.values[id as usize]
                .fields
                .iter()
                .any(|field| has_inline_layout_cycle(module, field.ty, visiting));
            visiting.remove(&key);
            cycle
        }
        IrType::Enum(id) => {
            let key = InlineLayout::Enum(id);
            if !visiting.insert(key) {
                return true;
            }
            let cycle = module.enums[id as usize]
                .fields
                .iter()
                .any(|field| has_inline_layout_cycle(module, field.ty, visiting));
            visiting.remove(&key);
            cycle
        }
        IrType::Closure(id) => {
            let key = InlineLayout::Closure(id);
            if !visiting.insert(key) {
                return true;
            }
            let cycle = module.closures[id as usize]
                .captures
                .iter()
                .any(|&capture| has_inline_layout_cycle(module, capture, visiting));
            visiting.remove(&key);
            cycle
        }
        IrType::Nullable(Nullable::Value(id)) => {
            has_inline_layout_cycle(module, IrType::Value(id), visiting)
        }
        IrType::Nullable(Nullable::Enum(id)) => {
            has_inline_layout_cycle(module, IrType::Enum(id), visiting)
        }
        _ => false,
    }
}

/// Validate every layout table before any later verifier/backend pass indexes
/// it to derive GC paths or LLVM field offsets.
fn verify_module_layouts(module: &Module, errors: &mut Vec<IrError>) {
    fn check_type(module: &Module, errors: &mut Vec<IrError>, ty: IrType, context: &str) {
        let valid = match ty {
            // Borrowed views are instruction-local capabilities, never
            // admissible in a module-level layout table.
            IrType::BorrowedView { .. } => false,
            IrType::Closure(id) | IrType::Callable(id) => id < module.closures.len() as u32,
            IrType::Channel(id) => id < module.channel_types.len() as u32,
            IrType::Mutex(id) => id < module.mutex_types.len() as u32,
            IrType::Object(id) | IrType::Nullable(Nullable::Object(id)) => {
                id < module.objects.len() as u32
            }
            IrType::Value(id) | IrType::Nullable(Nullable::Value(id)) => {
                id < module.values.len() as u32
            }
            IrType::Enum(id) | IrType::Nullable(Nullable::Enum(id)) => {
                id < module.enums.len() as u32
            }
            IrType::Nullable(Nullable::Union(id)) => {
                id < module.enums.len() as u32
                    && module
                        .union_types
                        .iter()
                        .any(|layout| layout.representation_layout == Some(id))
            }
            IrType::Pointer(id) => id < module.pointer_types.len() as u32,
            IrType::Weak(id) => id < module.weak_types.len() as u32,
            IrType::NativeSlice(id) => id < module.native_slice_types.len() as u32,
            IrType::NativeSliceMut(id) => id < module.native_slice_mut_types.len() as u32,
            IrType::Dependent(id) => id < module.dependent_types.len() as u32,
            IrType::Pin(id) => id < module.pin_types.len() as u32,
            IrType::Array(id) | IrType::Nullable(Nullable::Array(id)) => {
                id < module.array_types.len() as u32
            }
            IrType::List(id) | IrType::Nullable(Nullable::List(id)) => {
                id < module.list_types.len() as u32
            }
            IrType::Atomic(_) | IrType::Nullable(Nullable::Atomic(_)) => true,
            IrType::Map(id) | IrType::Nullable(Nullable::Map(id)) => {
                id < module.map_types.len() as u32
            }
            IrType::Set(id) | IrType::Nullable(Nullable::Set(id)) => {
                id < module.set_types.len() as u32
            }
            IrType::Union(id) => {
                id < module.enums.len() as u32
                    && module
                        .union_types
                        .iter()
                        .any(|layout| layout.representation_layout == Some(id))
            }
            IrType::Nullable(Nullable::Contract(_)) => true,
            IrType::ObjectAny
            | IrType::Dynamic
            | IrType::Nullable(Nullable::ObjectAny | Nullable::Dynamic) => true,
            IrType::Void
            | IrType::Never
            | IrType::Int(_)
            | IrType::Float(_)
            | IrType::Decimal
            | IrType::Boolean
            | IrType::String
            | IrType::Regex
            | IrType::Char
            | IrType::Job
            | IrType::Contract(_)
            | IrType::Nullable(
                Nullable::Int(_)
                | Nullable::Float(_)
                | Nullable::Boolean
                | Nullable::String
                | Nullable::Char,
            )
            | IrType::JournalHandle
            | IrType::Range => true,
        };
        if !valid {
            module_error(
                errors,
                format!(
                    "{context} references an out-of-range layout in {} ({ty:?})",
                    ty.as_str()
                ),
            );
        }
    }
    let mutex_callbacks = mutex_callback_functions(module);
    let mutex_callback_layouts: HashSet<u32> = module
        .functions
        .iter()
        .flat_map(|f| f.blocks.iter())
        .flat_map(|b| b.instructions.iter())
        .filter_map(|inst| {
            if let InstKind::MutexWithCall { layout, .. } = &inst.kind {
                Some(*layout)
            } else {
                None
            }
        })
        .collect();
    let mut layout_errors = Vec::new();

    for (index, layout) in module.objects.iter().enumerate() {
        for field in &layout.fields {
            check_type(
                module,
                errors,
                field.ty,
                &format!("object layout {index} field `{}`", field.name),
            );
        }
    }
    for (index, layout) in module.values.iter().enumerate() {
        for field in &layout.fields {
            check_type(
                module,
                errors,
                field.ty,
                &format!("value layout {index} field `{}`", field.name),
            );
        }
    }
    let mut union_representations = HashSet::new();
    for (index, layout) in module.union_types.iter().enumerate() {
        if layout.validate().is_err() {
            module_error(
                errors,
                format!(
                    "union layout {index} has noncanonical alternatives or an invalid fingerprint"
                ),
            );
        }
        for ty in &layout.alternatives {
            check_type(
                module,
                errors,
                *ty,
                &format!("union layout {index} alternative"),
            );
        }
        if let Some(enum_id) = layout.representation_layout {
            if !union_representations.insert(enum_id) {
                module_error(
                    errors,
                    format!("union layout {index} reuses bridge enum layout {enum_id}"),
                );
            }
            match module.enums.get(enum_id as usize) {
                None => module_error(
                    errors,
                    format!("union layout {index} references missing bridge enum layout {enum_id}"),
                ),
                Some(enum_layout) => {
                    match module.union_layout_descriptor(index as u32) {
                        Ok(expected) if enum_layout.union_layout.as_ref() != Some(&expected) => {
                            module_error(
                                errors,
                                format!(
                                    "union layout {index} bridge enum has missing or inconsistent physical layout metadata"
                                ),
                            );
                        }
                        Err(crate::UnionMetadataError::InlinePayloadTooLarge(_))
                            if enum_layout.union_layout.is_some() =>
                        {
                            module_error(
                                errors,
                                format!(
                                    "union layout {index} oversized bridge enum must not carry an inline layout descriptor"
                                ),
                            );
                        }
                        Err(crate::UnionMetadataError::InlinePayloadTooLarge(_)) => {}
                        Err(error) => module_error(
                            errors,
                            format!("union layout {index} physical layout is invalid: {error}"),
                        ),
                        _ => {}
                    }
                    if enum_layout.variants.len() != layout.alternatives.len() {
                        module_error(
                            errors,
                            format!(
                                "union layout {index} bridge enum has {} variants for {} alternatives",
                                enum_layout.variants.len(),
                                layout.alternatives.len()
                            ),
                        );
                    }
                    for (alternative, expected) in layout.alternatives.iter().enumerate() {
                        let Some(fields) = enum_layout.variants.get(alternative) else {
                            continue;
                        };
                        if fields.len() != 1
                            || enum_layout
                                .fields
                                .get(fields[0] as usize)
                                .is_none_or(|field| field.ty != *expected)
                        {
                            module_error(
                                errors,
                                format!(
                                    "union layout {index} alternative {alternative} does not match its bridge enum payload: expected {expected:?}, got {:?}",
                                    fields
                                        .first()
                                        .and_then(|field| enum_layout.fields.get(*field as usize))
                                        .map(|field| field.ty)
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
    for (index, layout) in module.enums.iter().enumerate() {
        for field in &layout.fields {
            check_type(
                module,
                errors,
                field.ty,
                &format!("enum layout {index} field `{}`", field.name),
            );
        }
        let mut assigned = vec![false; layout.fields.len()];
        for (variant_index, variant) in layout.variants.iter().enumerate() {
            for &field_index in variant {
                let Some(assigned_slot) = assigned.get_mut(field_index as usize) else {
                    layout_errors.push(format!(
                        "enum layout {index} variant {variant_index} has out-of-range field index {field_index}"
                    ));
                    continue;
                };
                if *assigned_slot {
                    layout_errors.push(format!(
                        "enum layout {index} reuses flattened field index {field_index}"
                    ));
                }
                *assigned_slot = true;
            }
        }
        if assigned.iter().any(|assigned| !assigned) {
            layout_errors.push(format!(
                "enum layout {index} contains a flattened field not owned by any variant"
            ));
        }
    }
    for (index, layout) in module.closures.iter().enumerate() {
        for &ty in &layout.captures {
            check_type(module, errors, ty, &format!("closure layout {index}"));
        }
        for (parameter_index, &ty) in layout.params.iter().enumerate() {
            if !(mutex_callback_layouts.contains(&(index as u32))
                && parameter_index == 0
                && matches!(ty, IrType::BorrowedView { .. }))
            {
                check_type(module, errors, ty, &format!("closure layout {index}"));
            }
        }
        check_type(
            module,
            errors,
            layout.returns,
            &format!("closure layout {index} return"),
        );
    }

    for (table_name, table) in [
        ("pointer", &module.pointer_types),
        ("mutex", &module.mutex_types),
        ("weak", &module.weak_types),
        ("native slice", &module.native_slice_types),
        ("mutable native slice", &module.native_slice_mut_types),
        ("dependent", &module.dependent_types),
        ("pin", &module.pin_types),
        ("array", &module.array_types),
        ("list", &module.list_types),
        ("set", &module.set_types),
    ] {
        for (index, &ty) in table.iter().enumerate() {
            check_type(
                module,
                errors,
                ty,
                &format!("{table_name} type table entry {index}"),
            );
        }
    }
    for (index, &ty) in module.mutex_types.iter().enumerate() {
        if matches!(
            ty,
            IrType::Void | IrType::Never | IrType::BorrowedView { .. }
        ) {
            module_error(
                errors,
                format!("mutex type table entry {index} has an unstorable payload type"),
            );
        }
    }
    for (index, &(key, value)) in module.map_types.iter().enumerate() {
        check_type(
            module,
            errors,
            key,
            &format!("map type table entry {index} key"),
        );
        check_type(
            module,
            errors,
            value,
            &format!("map type table entry {index} value"),
        );
    }
    for (index, function) in module.functions.iter().enumerate() {
        check_type(
            module,
            errors,
            function.return_type,
            &format!("function {index} return"),
        );
        for (slot_index, slot) in function.slots.iter().enumerate() {
            let slot_id = SlotId(slot_index as u32);
            let callback_parameter = mutex_callbacks.contains(&function.name)
                && function.params.contains(&slot_id)
                && matches!(slot.ty, IrType::BorrowedView { .. });
            if !callback_parameter {
                check_type(
                    module,
                    errors,
                    slot.ty,
                    &format!("function {index} slot {slot_index}"),
                );
            }
        }
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                if matches!(
                    (&instruction.kind, instruction.ty),
                    (
                        InstKind::BorrowViewStart { region, .. },
                        IrType::BorrowedView { region: ty_region, .. }
                    ) if *region == ty_region
                ) || matches!(
                    (&instruction.kind, instruction.ty),
                    (InstKind::Load(slot), IrType::BorrowedView { .. })
                        if mutex_callbacks.contains(&function.name)
                            && function.params.contains(slot)
                            && function.slot(*slot).is_some_and(|s| s.ty == instruction.ty)
                ) {
                    continue;
                }
                check_type(
                    module,
                    errors,
                    instruction.ty,
                    &format!(
                        "function {index} block {block_index} instruction {instruction_index} ({:?})",
                        instruction.kind
                    ),
                );
            }
        }
    }
    for (index, function) in module.externs.iter().enumerate() {
        for &ty in &function.params {
            check_type(
                module,
                errors,
                ty,
                &format!("extern function {index} parameter"),
            );
        }
        check_type(
            module,
            errors,
            function.return_type,
            &format!("extern function {index} return"),
        );
    }
    for detail in layout_errors {
        module_error(errors, detail);
    }
    if errors.is_empty() {
        for (index, layout) in module.values.iter().enumerate() {
            if layout
                .fields
                .iter()
                .any(|field| has_inline_layout_cycle(module, field.ty, &mut HashSet::new()))
            {
                module_error(
                    errors,
                    format!("value layout {index} contains an infinitely recursive inline field"),
                );
            }
        }
        for (index, layout) in module.enums.iter().enumerate() {
            if layout
                .fields
                .iter()
                .any(|field| has_inline_layout_cycle(module, field.ty, &mut HashSet::new()))
            {
                module_error(
                    errors,
                    format!("enum layout {index} contains an infinitely recursive inline field"),
                );
            }
        }
        for (index, layout) in module.closures.iter().enumerate() {
            if layout
                .captures
                .iter()
                .any(|&ty| has_inline_layout_cycle(module, ty, &mut HashSet::new()))
            {
                module_error(
                    errors,
                    format!(
                        "closure layout {index} contains an infinitely recursive inline capture"
                    ),
                );
            }
        }
    }
}

fn verify_function(
    module: &Module,
    function: &Function,
    direct_effects: &HashMap<String, bool>,
    mutex_callback_functions: &HashSet<String>,
    errors: &mut Vec<IrError>,
) {
    let mut report = |detail: String| {
        errors.push(IrError {
            function: function.name.clone(),
            detail,
        })
    };

    let mut seen_roots = std::collections::HashSet::new();
    for &root in &function.gc_roots {
        match function.slots.get(root.0 as usize) {
            None => report(format!(
                "GC root {root:?} does not refer to a function slot"
            )),
            Some(slot) => {
                if !slot.ty.is_managed_reference(module) {
                    report(format!(
                        "GC root {root:?} has unmanaged type {}",
                        slot.ty.as_str()
                    ));
                }
                if !seen_roots.insert(root) {
                    report(format!("GC root {root:?} is listed more than once"));
                }
            }
        }
    }
    for (index, slot) in function.slots.iter().enumerate() {
        let slot_id = SlotId(index as u32);
        if matches!(slot.ty, IrType::BorrowedView { .. })
            && (!mutex_callback_functions.contains(&function.name)
                || !function.params.contains(&SlotId(index as u32)))
        {
            report(format!("slot {slot_id:?} cannot contain a BorrowedView"));
        }
        if slot.ty.is_managed_reference(module) && !seen_roots.contains(&slot_id) {
            report(format!(
                "managed slot {slot_id:?} is missing from the GC roots"
            ));
        }
    }

    let has_borrowed_param = function.params.iter().any(|slot| {
        function
            .slot(*slot)
            .is_some_and(|s| matches!(s.ty, IrType::BorrowedView { .. }))
    });
    if has_borrowed_param && !mutex_callback_functions.contains(&function.name) {
        report("BorrowedView function parameters are allowed only for a dedicated MutexWithCall callback".into());
    }

    if function.block(function.entry).is_none() {
        report(format!(
            "the entry block {:?} does not exist",
            function.entry
        ));
        return;
    }

    let block_ids: Vec<BlockId> = function.blocks.iter().map(|b| b.id).collect();

    // Every definition in the function, collected up front so the verifier can
    // tell a value that does not exist from one defined further down: they are
    // different defects and deserve different messages.
    let mut declared: HashMap<ValueId, BlockId> = HashMap::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            if let Some(result) = inst.result {
                declared.entry(result).or_insert(block.id);
            }
        }
    }

    // Value types, and the block each was defined in. In this design values do
    // not cross blocks: locals travel through slots, which is what allows
    // skipping SSA and phi nodes.
    let mut values: HashMap<ValueId, (BlockId, IrType)> = HashMap::new();
    let mut borrow_regions = HashSet::new();

    for block in &function.blocks {
        let mut defined_here: Vec<ValueId> = Vec::new();
        let mut open_borrows: HashMap<ValueId, u32> = HashMap::new();

        for (index, inst) in block.instructions.iter().enumerate() {
            let position = format!("block {:?}, instruction {index}", block.id);

            match &inst.kind {
                InstKind::BorrowViewStart { region, mutex } => {
                    if !borrow_regions.insert(*region) {
                        report(format!(
                            "{position}: borrow region {region} is started more than once"
                        ));
                    }
                    let expected_view = values
                        .get(&mutex.0)
                        .and_then(|(_, ty)| match ty {
                            IrType::Mutex(mutex_type) => Some(*mutex_type),
                            _ => None,
                        })
                        .map(|mutex_type| IrType::BorrowedView {
                            region: *region,
                            mutex_type,
                        });
                    if expected_view != Some(inst.ty) {
                        report(format!(
                            "{position}: BorrowViewStart must produce a BorrowedView with the same region and payload type as its Mutex handle"
                        ));
                    }
                    match inst.result {
                        Some(view) => {
                            if open_borrows.insert(view, *region).is_some() {
                                report(format!(
                                    "{position}: BorrowViewStart reuses view value {view:?}"
                                ));
                            }
                        }
                        None => report(format!(
                            "{position}: BorrowViewStart must produce a view value"
                        )),
                    }
                    if !values.get(&mutex.0).is_some_and(|(_, ty)| matches!(ty, IrType::Mutex(id) if *id < module.mutex_types.len() as u32)) {
                        report(format!(
                            "{position}: BorrowViewStart requires a valid typed Mutex handle"
                        ));
                    }
                }
                InstKind::BorrowViewEnd { view } => {
                    expect(
                        inst.ty,
                        IrType::Void,
                        &position,
                        "BorrowViewEnd",
                        &mut report,
                    );
                    if inst.result.is_some() {
                        report(format!("{position}: BorrowViewEnd cannot produce a result"));
                    }
                    match values.get(&view.0).map(|(_, ty)| *ty) {
                        Some(IrType::BorrowedView { region, .. }) => {
                            if open_borrows.remove(&view.0) != Some(region) {
                                report(format!(
                                    "{position}: BorrowViewEnd closes unknown, already closed, or mismatched view {:?}",
                                    view.0
                                ));
                            }
                        }
                        Some(ty) => report(format!(
                            "{position}: BorrowViewEnd operand is {}, expected BorrowedView",
                            ty.as_str()
                        )),
                        None => report(format!(
                            "{position}: BorrowViewEnd operand is undefined or not yet defined"
                        )),
                    }
                }
                InstKind::BorrowViewLoad { view }
                | InstKind::BorrowViewStore { view, .. }
                | InstKind::BorrowViewFieldLoad { view, .. }
                | InstKind::BorrowViewFieldStore { view, .. } => {
                    match values.get(&view.0).map(|(_, ty)| *ty) {
                        Some(IrType::BorrowedView { region, mutex_type }) => {
                            let active = open_borrows.get(&view.0) == Some(&region)
                                || function.params.iter().any(|slot| {
                                    function.slot(*slot).is_some_and(|s| {
                                        s.ty == IrType::BorrowedView { region, mutex_type }
                                    })
                                });
                            if !active {
                                report(format!(
                                    "{position}: borrowed payload access requires an active matching region"
                                ));
                            }
                        }
                        _ => report(format!(
                            "{position}: borrowed payload access requires a BorrowedView"
                        )),
                    }
                }
                _ => {}
            }

            if matches!(inst.ty, IrType::BorrowedView { .. })
                && !matches!(inst.kind, InstKind::BorrowViewStart { .. })
                && !matches!(&inst.kind, InstKind::Load(slot) if has_borrowed_param && function.params.contains(slot) && function.slot(*slot).is_some_and(|s| s.ty == inst.ty))
            {
                report(format!(
                    "{position}: only BorrowViewStart may produce a BorrowedView"
                ));
            }

            if (!open_borrows.is_empty() || has_borrowed_param)
                && may_suspend_or_spawn(&inst.kind, direct_effects)
            {
                report(format!(
                    "{position}: possibly suspending, blocking, spawning, performing host I/O, or crossing a GC/runtime boundary is forbidden while a BorrowedView is live"
                ));
            }

            for operand in operands_of(&inst.kind) {
                if !matches!(
                    inst.kind,
                    InstKind::BorrowViewEnd { .. }
                        | InstKind::BorrowViewLoad { .. }
                        | InstKind::BorrowViewStore { .. }
                        | InstKind::BorrowViewFieldLoad { .. }
                        | InstKind::BorrowViewFieldStore { .. }
                ) && values
                    .get(&operand.0)
                    .is_some_and(|(_, ty)| matches!(ty, IrType::BorrowedView { .. }))
                {
                    report(format!(
                        "{position}: BorrowedView {:?} may only be consumed by borrow load/store/end",
                        operand.0
                    ));
                }
                if defined_here.contains(&operand.0) {
                    continue;
                }

                match declared.get(&operand.0) {
                    None => report(format!("{position}: uses undefined value {:?}", operand.0)),
                    Some(origin) if *origin != block.id => report(format!(
                        "{position}: uses value {:?} defined in another block ({origin:?}); \
                         values do not cross blocks in this IR",
                        operand.0
                    )),
                    Some(_) => report(format!(
                        "{position}: uses value {:?} before it is defined",
                        operand.0
                    )),
                }
            }

            verify_instruction(module, function, inst, &values, &position, &mut report);

            if let Some(result) = inst.result {
                if values.contains_key(&result) {
                    report(format!("{position}: value {result:?} is defined twice"));
                }
                values.insert(result, (block.id, inst.ty));
                defined_here.push(result);
            }
        }

        // Every block ends in exactly one terminator (design D2).
        let Some(terminator) = &block.terminator else {
            report(format!("block {:?} has no terminator", block.id));
            continue;
        };

        for successor in terminator.successors() {
            if !block_ids.contains(&successor) {
                report(format!(
                    "block {:?} jumps to {successor:?}, which does not exist",
                    block.id
                ));
            }
        }

        verify_terminator(module, function, block, terminator, &values, &mut report);

        if !open_borrows.is_empty() {
            let mut regions: Vec<_> = open_borrows.values().copied().collect();
            regions.sort_unstable();
            report(format!(
                "block {:?} leaves borrow region(s) open: {regions:?}",
                block.id
            ));
        }
    }

    if matches!(function.return_type, IrType::BorrowedView { .. }) {
        report("function return type cannot be BorrowedView".into());
    }
    verify_mutex_lock_dataflow(module, function, &mut report);
}

/// Enforces consistent lock ownership over every reachable control-flow path.
fn verify_mutex_lock_dataflow(
    module: &Module,
    function: &Function,
    report: &mut impl FnMut(String),
) {
    let blocks: HashMap<BlockId, &Block> = function.blocks.iter().map(|b| (b.id, b)).collect();
    let mut incoming: HashMap<BlockId, BTreeMap<MutexId, SlotId>> = HashMap::new();
    let mut queue = VecDeque::new();
    incoming.insert(function.entry, BTreeMap::new());
    queue.push_back(function.entry);

    while let Some(block_id) = queue.pop_front() {
        let Some(block) = blocks.get(&block_id) else {
            continue;
        };
        let mut held = incoming[&block_id].clone();
        let mut loaded_mutex_slots = HashMap::new();
        let mut active_views: HashMap<ValueId, (MutexId, SlotId)> = HashMap::new();
        for (index, instruction) in block.instructions.iter().enumerate() {
            let position = format!("block {block_id:?}, instruction {index}");
            match instruction.kind {
                InstKind::Load(slot) => {
                    if let Some(result) = instruction.result
                        && function.slot(slot).is_some_and(|candidate| {
                            matches!(candidate.ty, IrType::Mutex(id) if id < module.mutex_types.len() as u32)
                        })
                    {
                        loaded_mutex_slots.insert(result, slot);
                    }
                }
                InstKind::MutexAcquire { id, mutex } => {
                    if held.insert(id, mutex).is_some() {
                        report(format!(
                            "{position}: mutex identity {id:?} is acquired while already held"
                        ));
                    }
                    if instruction.result.is_some() || instruction.ty != IrType::Void {
                        report(format!(
                            "{position}: MutexAcquire must be a result-less Void instruction"
                        ));
                    }
                }
                InstKind::MutexRelease { id, mutex } => {
                    if active_views
                        .values()
                        .any(|(_, view_slot)| *view_slot == mutex)
                    {
                        report(format!(
                            "{position}: cannot release mutex {id:?} while a BorrowedView is active"
                        ));
                    }
                    match held.remove(&id) {
                        None => report(format!(
                            "{position}: mutex identity {id:?} is released without being held"
                        )),
                        Some(acquired) if acquired != mutex => report(format!(
                            "{position}: MutexRelease handle does not match the handle acquired for {id:?}"
                        )),
                        Some(_) => {}
                    }
                    if instruction.result.is_some() || instruction.ty != IrType::Void {
                        report(format!(
                            "{position}: MutexRelease must be a result-less Void instruction"
                        ));
                    }
                }
                InstKind::Store(slot, _) if held.values().any(|locked| *locked == slot) => {
                    report(format!(
                        "{position}: cannot replace mutex handle in slot {slot:?} while it is held"
                    ));
                }
                InstKind::BorrowViewStart { mutex, .. } => {
                    let loaded_slot = loaded_mutex_slots.get(&mutex.0).copied();
                    let owner = loaded_slot.and_then(|slot| {
                        held.iter()
                            .find_map(|(id, locked_slot)| (*locked_slot == slot).then_some((*id, slot)))
                    });
                    match owner {
                        Some((id, slot)) => {
                            if let Some(result) = instruction.result {
                                active_views.insert(result, (id, slot));
                            }
                        }
                        None => report(format!(
                            "{position}: BorrowViewStart requires its Mutex handle to be loaded from a slot whose lock is currently held"
                        )),
                    }
                }
                InstKind::BorrowViewEnd { view } => {
                    active_views.remove(&view.0);
                }
                _ => {}
            }
        }

        let Some(terminator) = &block.terminator else {
            continue;
        };
        let successors = terminator.successors();
        if successors.is_empty() {
            if !held.is_empty() {
                report(format!(
                    "reachable exit from block {block_id:?} retains acquired mutexes {held:?}"
                ));
            }
            continue;
        }
        for successor in successors {
            if !blocks.contains_key(&successor) {
                return;
            }
            match incoming.get(&successor) {
                None => {
                    incoming.insert(successor, held.clone());
                    queue.push_back(successor);
                }
                Some(previous) if previous != &held => report(format!(
                    "control-flow join at block {successor:?} has incompatible mutex states: {previous:?} versus {held:?}"
                )),
                Some(_) => {}
            }
        }
    }
}

fn verify_instruction(
    module: &Module,
    function: &Function,
    inst: &Instruction,
    values: &HashMap<ValueId, (BlockId, IrType)>,
    position: &str,
    report: &mut impl FnMut(String),
) {
    let type_of = |operand: &Operand| values.get(&operand.0).map(|(_, ty)| *ty);

    // A `Void` instruction produces no value, so declaring a result id is
    // contradictory: consumers read `result.is_some()` as "this instruction
    // materializes a value" (the interpreter derives its return destination
    // from exactly that). The one exception is `Undefined`, the lowering's
    // placeholder for a `Void` expression forced into value position — it
    // does materialize `Value::Undefined`. `Never` stays allowed too:
    // `FatalError` declares a result purely as a diverging-branch marker.
    if inst.ty == IrType::Void && inst.result.is_some() && !matches!(inst.kind, InstKind::Undefined)
    {
        report(format!(
            "{position}: a Void instruction cannot declare a result value"
        ));
    }

    match &inst.kind {
        InstKind::MutexAcquire { mutex, .. } | InstKind::MutexRelease { mutex, .. } => {
            expect(inst.ty, IrType::Void, position, "mutex operation", report);
            if !function.slot(*mutex).is_some_and(
                |slot| matches!(slot.ty, IrType::Mutex(id) if id < module.mutex_types.len() as u32),
            ) {
                report(format!(
                    "{position}: mutex operation requires a valid Mutex handle slot"
                ));
            }
            if inst.result.is_some() {
                report(format!(
                    "{position}: mutex operations cannot produce a result"
                ));
            }
        }
        InstKind::BorrowViewStart { region, mutex } => {
            let expected_view = type_of(mutex).and_then(|ty| match ty {
                IrType::Mutex(mutex_type) if mutex_type < module.mutex_types.len() as u32 => {
                    Some(IrType::BorrowedView {
                        region: *region,
                        mutex_type,
                    })
                }
                _ => None,
            });
            if expected_view != Some(inst.ty) {
                report(format!(
                    "{position}: BorrowViewStart requires a valid typed Mutex handle and matching BorrowedView payload type"
                ));
            }
            if inst.result.is_none() {
                report(format!("{position}: BorrowViewStart must produce a result"));
            }
        }
        InstKind::BorrowViewEnd { .. } => {
            expect(inst.ty, IrType::Void, position, "BorrowViewEnd", report);
            if inst.result.is_some() {
                report(format!("{position}: BorrowViewEnd cannot produce a result"));
            }
        }
        InstKind::BorrowViewLoad { view } => {
            let expected = type_of(view).and_then(|ty| match ty {
                IrType::BorrowedView { mutex_type, .. } => {
                    module.mutex_types.get(mutex_type as usize).copied()
                }
                _ => None,
            });
            if expected.is_none() {
                report(format!(
                    "{position}: BorrowViewLoad requires a BorrowedView"
                ));
            }
            if expected != Some(inst.ty) {
                report(format!(
                    "{position}: BorrowViewLoad result must have the exact mutex payload type"
                ));
            }
            if inst.result.is_none() {
                report(format!("{position}: BorrowViewLoad must produce a value"));
            }
        }
        InstKind::BorrowViewStore { view, value } => {
            expect(inst.ty, IrType::Void, position, "BorrowViewStore", report);
            let expected = type_of(view).and_then(|ty| match ty {
                IrType::BorrowedView { mutex_type, .. } => {
                    module.mutex_types.get(mutex_type as usize).copied()
                }
                _ => None,
            });
            if expected.is_none() {
                report(format!(
                    "{position}: BorrowViewStore requires a BorrowedView"
                ));
            }
            if type_of(value) != expected {
                report(format!(
                    "{position}: BorrowViewStore value must have the exact mutex payload type"
                ));
            }
            if inst.result.is_some() {
                report(format!(
                    "{position}: BorrowViewStore cannot produce a result"
                ));
            }
        }
        InstKind::BorrowViewFieldLoad { view, path } => {
            let actual = type_of(view).and_then(|ty| match ty {
                IrType::BorrowedView { mutex_type, .. } => {
                    module.mutex_types.get(mutex_type as usize).copied()
                }
                _ => None,
            });
            match actual.and_then(|ty| resolve_borrowed_value_path(module, ty, path)) {
                Some(field_ty) if field_ty == inst.ty => {}
                Some(field_ty) => report(format!(
                    "{position}: BorrowViewFieldLoad result is {}, expected {}",
                    inst.ty.as_str(),
                    field_ty.as_str()
                )),
                None => report(format!(
                    "{position}: BorrowViewFieldLoad path is invalid or crosses an Enum without a variant guard"
                )),
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: BorrowViewFieldLoad must produce a value"
                ));
            }
        }
        InstKind::BorrowViewFieldStore { view, path, value } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "BorrowViewFieldStore",
                report,
            );
            let actual = type_of(view).and_then(|ty| match ty {
                IrType::BorrowedView { mutex_type, .. } => {
                    module.mutex_types.get(mutex_type as usize).copied()
                }
                _ => None,
            });
            match actual.and_then(|ty| resolve_borrowed_value_path(module, ty, path)) {
                Some(field_ty) if type_of(value) == Some(field_ty) => {}
                Some(field_ty) => report(format!(
                    "{position}: BorrowViewFieldStore value must have exact type {}",
                    field_ty.as_str()
                )),
                None => report(format!(
                    "{position}: BorrowViewFieldStore path is invalid or crosses an Enum without a variant guard"
                )),
            }
            if inst.result.is_some() {
                report(format!(
                    "{position}: BorrowViewFieldStore cannot produce a result"
                ));
            }
        }
        InstKind::MutexWithCall {
            mutex,
            callback,
            layout,
            result_slot,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "MutexWithCall",
                report,
            );
            let mutex_type = type_of(mutex).and_then(|ty| {
                if let IrType::Mutex(id) = ty {
                    Some(id)
                } else {
                    None
                }
            });
            if mutex_type.is_none_or(|id| id as usize >= module.mutex_types.len()) {
                report(format!(
                    "{position}: MutexWithCall requires a valid Mutex handle"
                ));
            }
            if type_of(callback) != Some(IrType::Callable(*layout)) {
                report(format!(
                    "{position}: callback must have Callable({layout}) type"
                ));
            }
            if let Some(callback_layout) = module.closures.get(*layout as usize) {
                let tied = callback_layout.params.len() == 1
                    && match (callback_layout.params[0], mutex_type) {
                        (IrType::BorrowedView { mutex_type, .. }, Some(id)) => mutex_type == id,
                        _ => false,
                    };
                if !tied {
                    report(format!(
                        "{position}: callback layout must take exactly one BorrowedView tied to the mutex payload"
                    ));
                }
                match (callback_layout.returns, result_slot) {
                    (IrType::Void, None) => {}
                    (IrType::Void, Some(_)) => report(format!(
                        "{position}: Void callback requires result_slot=None"
                    )),
                    (_, None) => report(format!(
                        "{position}: non-Void callback requires a result slot"
                    )),
                    (returns, Some(slot)) => match function.slot(*slot) {
                        Some(info) if info.ty == returns => {
                            if returns.is_managed_reference(module)
                                && !function.gc_roots.contains(slot)
                            {
                                report(format!(
                                    "{position}: managed callback result slot must be a GC root"
                                ));
                            }
                        }
                        Some(info) => report(format!(
                            "{position}: callback result slot has {}, expected {}",
                            info.ty.as_str(),
                            returns.as_str()
                        )),
                        None => report(format!(
                            "{position}: callback result slot {slot:?} does not exist"
                        )),
                    },
                }
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: MutexWithCall must produce its runtime status"
                ));
            }
        }
        InstKind::MutexNew {
            payload,
            mutex_type,
            object_slot,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "MutexNew",
                report,
            );
            match module.mutex_types.get(*mutex_type as usize) {
                Some(payload_ty) if type_of(payload) == Some(*payload_ty) => {}
                Some(payload_ty) => report(format!(
                    "{position}: MutexNew payload type does not exactly match {}",
                    payload_ty.as_str()
                )),
                None => report(format!(
                    "{position}: MutexNew requires a valid mutex payload type"
                )),
            }
            match function.slot(*object_slot) {
                Some(slot) if slot.ty == IrType::Mutex(*mutex_type) => {
                    if !function.gc_roots.contains(object_slot) {
                        report(format!(
                            "{position}: MutexNew destination must be a GC root"
                        ));
                    }
                }
                Some(_) => report(format!(
                    "{position}: MutexNew destination slot must be Mutex({mutex_type})"
                )),
                None => report(format!(
                    "{position}: MutexNew destination slot does not exist"
                )),
            }
            if inst.result.is_none() {
                report(format!("{position}: MutexNew must produce runtime status"));
            }
        }
        InstKind::ProjectGlobalInit { global, value_slot } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ProjectGlobalInit",
                report,
            );
            match module.project_globals.get(*global as usize) {
                Some(layout) => match function.slot(*value_slot) {
                    Some(slot) if slot.ty == layout.payload => {
                        if layout.payload.is_managed_reference(module)
                            && !function.gc_roots.contains(value_slot)
                        {
                            report(format!(
                                "{position}: ProjectGlobalInit staging slot must be a GC root"
                            ));
                        }
                    }
                    Some(slot) => report(format!(
                        "{position}: ProjectGlobalInit staging slot has {}, expected {}",
                        slot.ty.as_str(),
                        layout.payload.as_str()
                    )),
                    None => report(format!(
                        "{position}: ProjectGlobalInit staging slot {value_slot:?} does not exist"
                    )),
                },
                None => report(format!(
                    "{position}: ProjectGlobalInit references missing project global {global}"
                )),
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: ProjectGlobalInit must produce runtime status"
                ));
            }
        }
        InstKind::ProjectGlobalGet { global, out } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ProjectGlobalGet",
                report,
            );
            match module.project_globals.get(*global as usize) {
                Some(layout) => match function.slot(*out) {
                    Some(slot) if slot.ty == layout.payload => {
                        if layout.payload.is_managed_reference(module)
                            && !function.gc_roots.contains(out)
                        {
                            report(format!(
                                "{position}: ProjectGlobalGet output slot must be a GC root"
                            ));
                        }
                    }
                    Some(slot) => report(format!(
                        "{position}: ProjectGlobalGet output slot has {}, expected {}",
                        slot.ty.as_str(),
                        layout.payload.as_str()
                    )),
                    None => report(format!(
                        "{position}: ProjectGlobalGet output slot {out:?} does not exist"
                    )),
                },
                None => report(format!(
                    "{position}: ProjectGlobalGet references missing project global {global}"
                )),
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: ProjectGlobalGet must produce runtime status"
                ));
            }
        }
        InstKind::ProjectGlobalSet {
            global,
            value,
            event,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ProjectGlobalSet",
                report,
            );
            match module.project_globals.get(*global as usize) {
                Some(layout) => {
                    if layout.readonly {
                        report(format!(
                            "{position}: ProjectGlobalSet cannot write read-only global `{}`",
                            layout.name
                        ));
                    }
                    if type_of(value) != Some(layout.payload) {
                        report(format!(
                            "{position}: ProjectGlobalSet value does not match {}",
                            layout.payload.as_str()
                        ));
                    }
                    match (layout.watch_change, event) {
                        (Some(IrType::Object(change_id)), Some(event)) => {
                            let object = &module.objects[change_id as usize];
                            match function.slot(event.object_slot) {
                                Some(slot) if slot.ty == IrType::Object(change_id) => {
                                    if !function.gc_roots.contains(&event.object_slot) {
                                        report(format!(
                                            "{position}: ProjectGlobalSet watch event slot must be a GC root"
                                        ));
                                    }
                                }
                                Some(slot) => report(format!(
                                    "{position}: ProjectGlobalSet watch event slot has {}, expected {}",
                                    slot.ty.as_str(),
                                    IrType::Object(change_id).as_str()
                                )),
                                None => report(format!(
                                    "{position}: ProjectGlobalSet watch event slot {:?} does not exist",
                                    event.object_slot
                                )),
                            }
                            for (index, expected, field_name) in [
                                (event.previous_field, layout.payload, "previous"),
                                (event.current_field, layout.payload, "current"),
                                (event.revision_field, IrType::Int(IntWidth::U64), "revision"),
                            ] {
                                if object.fields.get(index as usize).map(|field| field.ty)
                                    != Some(expected)
                                {
                                    report(format!(
                                        "{position}: ProjectGlobalSet watch {field_name} field does not match its declared layout"
                                    ));
                                }
                            }
                        }
                        (None, None) => {}
                        (Some(_), None) => report(format!(
                            "{position}: ProjectGlobalSet is missing its watched event object"
                        )),
                        (None, Some(_)) => report(format!(
                            "{position}: ProjectGlobalSet has an event for an unwatched global"
                        )),
                        (Some(other), Some(_)) => report(format!(
                            "{position}: ProjectGlobalSet watch event type is {}, expected object",
                            other.as_str()
                        )),
                    }
                }
                None => report(format!(
                    "{position}: ProjectGlobalSet references missing project global {global}"
                )),
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: ProjectGlobalSet must produce runtime status"
                ));
            }
        }
        InstKind::ProjectGlobalWatchSubscribe {
            global,
            channel,
            snapshot_out,
            token_out,
            revision_out,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ProjectGlobalWatchSubscribe",
                report,
            );
            if !matches!(type_of(channel), Some(IrType::Channel(_))) {
                report(format!(
                    "{position}: ProjectGlobalWatchSubscribe channel is not a Channel"
                ));
            }
            match module.project_globals.get(*global as usize) {
                Some(layout) => match function.slot(*snapshot_out) {
                    Some(slot) if slot.ty == layout.payload => {
                        if layout.payload.is_managed_reference(module)
                            && !function.gc_roots.contains(snapshot_out)
                        {
                            report(format!(
                                "{position}: ProjectGlobalWatchSubscribe snapshot slot must be a GC root"
                            ));
                        }
                    }
                    Some(slot) => report(format!(
                        "{position}: ProjectGlobalWatchSubscribe snapshot slot has {}, expected {}",
                        slot.ty.as_str(),
                        layout.payload.as_str()
                    )),
                    None => report(format!(
                        "{position}: ProjectGlobalWatchSubscribe snapshot slot {snapshot_out:?} does not exist"
                    )),
                },
                None => report(format!(
                    "{position}: ProjectGlobalWatchSubscribe references missing project global {global}"
                )),
            }
            for (slot_id, label) in [(*token_out, "token"), (*revision_out, "revision")] {
                match function.slot(slot_id) {
                    Some(slot) if slot.ty == IrType::Int(IntWidth::U64) => {}
                    Some(slot) => report(format!(
                        "{position}: ProjectGlobalWatchSubscribe {label} slot has {}, expected U64",
                        slot.ty.as_str()
                    )),
                    None => report(format!(
                        "{position}: ProjectGlobalWatchSubscribe {label} slot {slot_id:?} does not exist"
                    )),
                }
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: ProjectGlobalWatchSubscribe must produce runtime status"
                ));
            }
        }
        InstKind::ProjectGlobalWatchNext { token, out } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ProjectGlobalWatchNext",
                report,
            );
            if type_of(token) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: ProjectGlobalWatchNext token must be U64"
                ));
            }
            match function.slot(*out) {
                Some(slot) if matches!(slot.ty, IrType::Object(_)) => {
                    if !function.gc_roots.contains(out) {
                        report(format!(
                            "{position}: ProjectGlobalWatchNext output slot must be a GC root"
                        ));
                    }
                }
                Some(slot) => report(format!(
                    "{position}: ProjectGlobalWatchNext output slot has {}, expected object reference",
                    slot.ty.as_str()
                )),
                None => report(format!(
                    "{position}: ProjectGlobalWatchNext output slot {out:?} does not exist"
                )),
            }
            if inst.result.is_none() {
                report(format!(
                    "{position}: ProjectGlobalWatchNext must produce runtime status"
                ));
            }
        }
        InstKind::ProjectGlobalWatchClose { token } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "ProjectGlobalWatchClose",
                report,
            );
            if type_of(token) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: ProjectGlobalWatchClose token must be U64"
                ));
            }
            if inst.result.is_some() {
                report(format!(
                    "{position}: ProjectGlobalWatchClose must not produce a value"
                ));
            }
        }
        InstKind::SafepointPoll => expect(inst.ty, IrType::Void, position, "SafepointPoll", report),
        InstKind::ConstInt(_) => {
            if !matches!(inst.ty, IrType::Int(_) | IrType::Enum(_)) {
                report(format!(
                    "{position}: ConstInt must produce an Int type, got {ty:?}",
                    ty = inst.ty
                ));
            }
        }
        InstKind::ConstBool(_) => expect(inst.ty, IrType::Boolean, position, "ConstBool", report),
        InstKind::ConstString(id) => {
            expect(inst.ty, IrType::String, position, "ConstString", report);
            if module.strings.get(id.0 as usize).is_none() {
                report(format!(
                    "{position}: string {id:?} is not in the module table"
                ));
            }
        }
        InstKind::ConstChar(id) => {
            expect(inst.ty, IrType::Char, position, "ConstChar", report);
            if module.strings.get(id.0 as usize).is_none() {
                report(format!(
                    "{position}: string {id:?} is not in the module table"
                ));
            }
        }

        InstKind::FatalError(message) => {
            expect(inst.ty, IrType::Never, position, "FatalError", report);
            if let Some(ty) = type_of(message)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: FatalError's message is {}, expected String",
                    ty.as_str()
                ));
            }
        }

        InstKind::Throw(exception) => {
            expect(inst.ty, IrType::Void, position, "Throw", report);
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: Throw's exception is {}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::SetSuppressed {
            exception,
            suppressed,
        } => {
            expect(inst.ty, IrType::Void, position, "SetSuppressed", report);
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: SetSuppressed's exception is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(suppressed)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: SetSuppressed's suppressed is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::StackTrace(exception) => {
            expect(inst.ty, IrType::String, position, "StackTrace", report);
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: StackTrace's exception is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::Suppressed(exception) => {
            if !matches!(inst.ty, IrType::Nullable(_)) {
                report(format!(
                    "{position}: Suppressed expects a nullable type, found {}",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(exception)
                && !matches!(ty, IrType::Object(_) | IrType::Contract(_))
            {
                report(format!(
                    "{position}: Suppressed's exception is {0}, neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::HasPendingException => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "HasPendingException",
                report,
            );
        }

        InstKind::IsCancelled => {
            expect(inst.ty, IrType::Boolean, position, "IsCancelled", report);
        }
        InstKind::CancelShieldEnter | InstKind::CancelShieldExit => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "cancellation shield",
                report,
            );
        }

        // No shape to check: `ty` is whichever class the matching `catch`
        // declared (roadmap Phase 4b) — see `InstKind::TakePendingException`'s
        // own doc comment.
        InstKind::TakePendingException => {}

        InstKind::StringGraphemeOffset { string, index } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I64),
                position,
                "StringGraphemeOffset",
                report,
            );
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: StringGraphemeOffset reads {}, which is not a String",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: StringGraphemeOffset's index is {}, expected Int64",
                    ty.as_str()
                ));
            }
        }

        InstKind::GraphemeLenAt { string, offset } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I64),
                position,
                "GraphemeLenAt",
                report,
            );
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: GraphemeLenAt reads {}, which is not a String",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(offset)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: GraphemeLenAt's offset is {}, expected Int64",
                    ty.as_str()
                ));
            }
        }

        InstKind::GraphemeSlice {
            string,
            offset,
            len,
        } => {
            expect(inst.ty, IrType::Char, position, "GraphemeSlice", report);
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: GraphemeSlice reads {}, which is not a String",
                    ty.as_str()
                ));
            }
            for (name, operand) in [("offset", offset), ("len", len)] {
                if let Some(ty) = type_of(operand)
                    && ty != IrType::Int(IntWidth::I64)
                {
                    report(format!(
                        "{position}: GraphemeSlice's {name} is {}, expected Int64",
                        ty.as_str()
                    ));
                }
            }
        }

        // No shape to check: `ty` is whatever the caller asked for, by
        // construction (roadmap Phase 4b's own `throw` early-return
        // placeholder) — see `InstKind::Undefined`'s own doc comment.
        InstKind::Undefined => {}

        InstKind::Alloc(id) => match module.objects.get(*id as usize) {
            None => report(format!(
                "{position}: allocates object layout {id}, which is not in the module table"
            )),
            Some(_) => expect(inst.ty, IrType::Object(*id), position, "Alloc", report),
        },

        InstKind::CheckedCast {
            object,
            target_class,
        } => match module.objects.get(*target_class as usize) {
            None => report(format!(
                "{position}: casts to object layout {target_class}, which is not in the module table"
            )),
            Some(_) => {
                expect(
                    inst.ty,
                    IrType::Object(*target_class),
                    position,
                    "CheckedCast",
                    report,
                );
                if let Some(ty) = type_of(object)
                    && !matches!(
                        ty,
                        IrType::Object(_) | IrType::Contract(_) | IrType::ObjectAny
                    )
                {
                    report(format!(
                        "{position}: casts {}, which is neither an object nor a contract",
                        ty.as_str()
                    ));
                }
            }
        },

        InstKind::IsInstance {
            object,
            target_class,
        } => {
            if module.objects.get(*target_class as usize).is_none() {
                report(format!(
                    "{position}: tests object layout {target_class}, which is not in the module table"
                ));
            }
            expect(inst.ty, IrType::Boolean, position, "IsInstance", report);
            if let Some(ty) = type_of(object)
                && !matches!(
                    ty,
                    IrType::Object(_) | IrType::Contract(_) | IrType::ObjectAny
                )
            {
                report(format!(
                    "{position}: tests {}, which is neither an object nor a contract",
                    ty.as_str()
                ));
            }
        }

        InstKind::Retype(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(
                    ty,
                    IrType::Object(_) | IrType::Contract(_) | IrType::ObjectAny
                )
            {
                report(format!(
                    "{position}: retypes {}, which is neither an object nor a contract",
                    ty.as_str()
                ));
            }
            if !matches!(
                inst.ty,
                IrType::Object(_) | IrType::Contract(_) | IrType::ObjectAny
            ) {
                report(format!(
                    "{position}: Retype declares {}, which is neither an object nor a contract",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::IntCast(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: IntCast converts {}, which is not an integer",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: IntCast declares {}, which is not an integer",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::ConstFloat(width, _) => expect(
            inst.ty,
            IrType::Float(*width),
            position,
            "ConstFloat",
            report,
        ),

        // `WordCast` reads a stored `i64` word back as the destination the
        // instruction itself declares (`universal-to-string-and-multiline-strings`).
        InstKind::WordCast(operand) => {
            if let Some(ty) = type_of(operand)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: WordCast converts {}, which is not an i64 word",
                    ty.as_str()
                ));
            }
        }

        InstKind::FloatCast(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Float(_))
            {
                report(format!(
                    "{position}: FloatCast converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Float(_)) {
                report(format!(
                    "{position}: FloatCast declares {}, which is not a Float",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::IntToFloat(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: IntToFloat converts {}, which is not an integer",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Float(_)) {
                report(format!(
                    "{position}: IntToFloat declares {}, which is not a Float",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::FloatToInt(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Float(_))
            {
                report(format!(
                    "{position}: FloatToInt converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: FloatToInt declares {}, which is not an integer",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::ConstDecimal(_) => {
            expect(inst.ty, IrType::Decimal, position, "ConstDecimal", report)
        }
        InstKind::IntToDecimal(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: IntToDecimal converts {}, which is not an integer",
                    ty.as_str()
                ));
            }
            expect(inst.ty, IrType::Decimal, position, "IntToDecimal", report);
        }
        InstKind::FloatToDecimal(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Float(_))
            {
                report(format!(
                    "{position}: FloatToDecimal converts {}, which is not a Float",
                    ty.as_str()
                ));
            }
            expect(inst.ty, IrType::Decimal, position, "FloatToDecimal", report);
        }
        InstKind::DecimalToInt(operand) => {
            if let Some(ty) = type_of(operand)
                && ty != IrType::Decimal
            {
                report(format!(
                    "{position}: DecimalToInt converts {}, which is not a Decimal",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Int(_)) {
                report(format!(
                    "{position}: DecimalToInt declares {}, which is not an integer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::DecimalToFloat(operand) => {
            if let Some(ty) = type_of(operand)
                && ty != IrType::Decimal
            {
                report(format!(
                    "{position}: DecimalToFloat converts {}, which is not a Decimal",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Float(_)) {
                report(format!(
                    "{position}: DecimalToFloat declares {}, which is not a Float",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::BuildValue { class, fields } => match module.values.get(*class as usize) {
            None => report(format!(
                "{position}: builds value layout {class}, which is not in the module table"
            )),
            Some(layout) => {
                expect(
                    inst.ty,
                    IrType::Value(*class),
                    position,
                    "BuildValue",
                    report,
                );
                if fields.len() != layout.fields.len() {
                    report(format!(
                        "{position}: builds {} with {} field{}, not {}",
                        layout.name,
                        fields.len(),
                        if fields.len() == 1 { "" } else { "s" },
                        layout.fields.len()
                    ));
                }
                for (field, given) in layout.fields.iter().zip(fields) {
                    if let Some(actual) = type_of(given)
                        && !types_compatible(module, actual, field.ty)
                    {
                        report(format!(
                            "{position}: builds field `{}` of type {} from a {}",
                            field.name,
                            field.ty.as_str(),
                            actual.as_str()
                        ));
                    }
                }
            }
        },

        InstKind::BuildEnum {
            enum_id,
            variant,
            fields,
        } => match module.enums.get(*enum_id as usize) {
            None => report(format!(
                "{position}: builds enum layout {enum_id}, which is not in the module table"
            )),
            Some(layout) => match layout.variants.get(*variant as usize) {
                None => report(format!(
                    "{position}: builds {}, which has no variant {variant}",
                    layout.name
                )),
                Some(indices) => {
                    expect(
                        inst.ty,
                        IrType::Enum(*enum_id),
                        position,
                        "BuildEnum",
                        report,
                    );
                    if fields.len() != indices.len() {
                        report(format!(
                            "{position}: builds variant {variant} of {} with {} field{}, not {}",
                            layout.name,
                            fields.len(),
                            if fields.len() == 1 { "" } else { "s" },
                            indices.len()
                        ));
                    }
                    for (&index, given) in indices.iter().zip(fields) {
                        let Some(field) = layout.fields.get(index as usize) else {
                            continue;
                        };
                        if let Some(actual) = type_of(given)
                            && !types_compatible(module, actual, field.ty)
                        {
                            report(format!(
                                "{position}: builds field `{}` of type {} from a {}",
                                field.name,
                                field.ty.as_str(),
                                actual.as_str()
                            ));
                        }
                    }
                }
            },
        },

        InstKind::UnionInject {
            union_id,
            alternative,
            payload,
        } => {
            let Some(layout) = module.union_types.get(*union_id as usize) else {
                report(format!(
                    "{position}: injects into missing union layout {union_id}"
                ));
                return;
            };
            let Some(representation) = layout.representation_layout else {
                report(format!(
                    "{position}: union layout {union_id} has no executable representation"
                ));
                return;
            };
            expect(
                inst.ty,
                IrType::Union(representation),
                position,
                "UnionInject",
                report,
            );
            match layout.alternatives.get(*alternative as usize) {
                None => report(format!(
                    "{position}: union layout {union_id} has no alternative {alternative}"
                )),
                Some(expected) => {
                    if let Some(actual) = type_of(payload)
                        && actual != *expected
                    {
                        report(format!(
                            "{position}: union alternative {alternative} expects {}, got {}",
                            expected.as_str(),
                            actual.as_str()
                        ));
                    }
                }
            }
        }
        InstKind::UnionTag(value) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U32),
                position,
                "UnionTag",
                report,
            );
            match type_of(value) {
                Some(IrType::Union(representation))
                    if module
                        .union_type_for_representation(representation)
                        .is_some() => {}
                Some(actual) => report(format!(
                    "{position}: UnionTag reads {}, expected a union",
                    actual.as_str()
                )),
                None => {}
            }
        }
        InstKind::UnionProject {
            value,
            union_id,
            alternative,
        } => {
            let Some(layout) = module.union_types.get(*union_id as usize) else {
                report(format!(
                    "{position}: projects from missing union layout {union_id}"
                ));
                return;
            };
            let Some(representation) = layout.representation_layout else {
                report(format!(
                    "{position}: union layout {union_id} has no executable representation"
                ));
                return;
            };
            if let Some(actual) = type_of(value)
                && actual != IrType::Union(representation)
            {
                report(format!(
                    "{position}: UnionProject reads {}, expected this union layout",
                    actual.as_str()
                ));
            }
            match layout.alternatives.get(*alternative as usize) {
                None => report(format!(
                    "{position}: union layout {union_id} has no alternative {alternative}"
                )),
                Some(expected) => expect(inst.ty, *expected, position, "UnionProject", report),
            }
        }

        InstKind::Discriminant(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "Discriminant",
                report,
            );
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Enum(_))
            {
                report(format!(
                    "{position}: reads the discriminant of {}, which is not an algebraic enum",
                    ty.as_str()
                ));
            }
        }

        InstKind::Concat { left, right } => {
            expect(inst.ty, IrType::String, position, "Concat", report);
            for operand in [left, right] {
                if let Some(ty) = type_of(operand)
                    && ty != IrType::String
                {
                    report(format!(
                        "{position}: concatenates {}, which is not a String",
                        ty.as_str()
                    ));
                }
            }
        }

        InstKind::Repeat { string, count } => {
            expect(inst.ty, IrType::String, position, "Repeat", report);
            if let Some(ty) = type_of(string)
                && ty != IrType::String
            {
                report(format!(
                    "{position}: repeats {}, which is not a String",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(count)
                && ty != IrType::Int(IntWidth::I32)
            {
                report(format!(
                    "{position}: repeats by {}, which is not a count",
                    ty.as_str()
                ));
            }
        }

        InstKind::CallContract {
            object,
            contract,
            index,
            specialization,
            ..
        } => {
            let reachable = match type_of(object) {
                Some(IrType::Contract(id)) => id == *contract,
                // A class reaches a contract it satisfies.
                Some(IrType::Object(id)) => module
                    .objects
                    .get(id as usize)
                    .is_some_and(|o| o.contracts.iter().any(|t| t.contract == *contract)),
                _ => false,
            };
            if !reachable {
                report(format!(
                    "{position}: calls method {index} of contract {contract} through {:?}, which does not satisfy it",
                    type_of(object)
                ));
            }
            if module
                .contracts
                .get(*contract as usize)
                .is_none_or(|layout| *index as usize >= layout.methods.len())
            {
                report(format!(
                    "{position}: calls missing method {index} of contract {contract}"
                ));
            }
            if let Some(specialization) = specialization {
                let has_slot = match type_of(object) {
                    Some(IrType::Object(id)) => module
                        .objects
                        .get(id as usize)
                        .and_then(|object| {
                            object
                                .contracts
                                .iter()
                                .find(|table| table.contract == *contract)
                        })
                        .is_some_and(|table| {
                            (*specialization as usize) < table.specializations.len()
                        }),
                    Some(IrType::Contract(_)) => module.objects.iter().any(|object| {
                        object
                            .contracts
                            .iter()
                            .find(|table| table.contract == *contract)
                            .is_some_and(|table| {
                                (*specialization as usize) < table.specializations.len()
                            })
                    }),
                    _ => false,
                };
                if !has_slot {
                    report(format!(
                        "{position}: contract {contract} has no generic specialization slot {specialization}"
                    ));
                }
            }
        }

        InstKind::CallVirtual { object, index, .. } => {
            let known = match type_of(object) {
                Some(IrType::Object(id)) => module
                    .objects
                    .get(id as usize)
                    .is_some_and(|o| (*index as usize) < o.methods.len()),
                _ => false,
            };
            if !known {
                report(format!(
                    "{position}: calls method {index} through something that is not an object with it"
                ));
            }
        }

        InstKind::LoadField { object, index } => {
            match field_type(module, type_of(object), *index) {
                Some(ty) => expect(inst.ty, ty, position, "LoadField", report),
                None => report(format!(
                    "{position}: reads field {index} of something that is not an object with it"
                )),
            }
        }

        InstKind::StoreField {
            object,
            index,
            value,
        } => {
            expect(inst.ty, IrType::Void, position, "StoreField", report);
            match field_type(module, type_of(object), *index) {
                Some(ty) => {
                    if let Some(actual) = type_of(value)
                        && actual != ty
                    {
                        report(format!(
                            "{position}: stores {} into a field of type {}",
                            actual.as_str(),
                            ty.as_str()
                        ));
                    }
                }
                None => report(format!(
                    "{position}: writes field {index} of something that is not an object with it"
                )),
            }
        }

        InstKind::Load(slot) => match function.slot(*slot) {
            None => report(format!(
                "{position}: reads slot {slot:?}, which does not exist"
            )),
            Some(s) => expect(inst.ty, s.ty, position, "Load", report),
        },

        InstKind::Store(slot, operand) => {
            expect(inst.ty, IrType::Void, position, "Store", report);
            match function.slot(*slot) {
                None => report(format!(
                    "{position}: writes slot {slot:?}, which does not exist"
                )),
                Some(s) => {
                    if let Some(value) = type_of(operand)
                        && value != s.ty
                    {
                        let actual_layout = match value {
                            IrType::Union(id) => module
                                .union_type_for_representation(id)
                                .map(|(_, layout)| &layout.alternatives),
                            _ => None,
                        };
                        let expected_layout = match s.ty {
                            IrType::Union(id) => module
                                .union_type_for_representation(id)
                                .map(|(_, layout)| &layout.alternatives),
                            _ => None,
                        };
                        report(format!(
                            "{position}: stores {} ({value:?}; alternatives {actual_layout:?}) into slot `{}` of type {} ({:?}; alternatives {expected_layout:?})",
                            value.as_str(),
                            s.name,
                            s.ty.as_str(),
                            s.ty
                        ));
                    }
                }
            }
        }

        InstKind::ObjectNew => expect(inst.ty, IrType::ObjectAny, position, "ObjectNew", report),

        InstKind::ObjectSeal { object } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ObjectSeal",
                report,
            );
            if let Some(actual) = type_of(object)
                && actual != IrType::ObjectAny
            {
                report(format!(
                    "{position}: ObjectSeal receiver is {}, expected Object",
                    actual.as_str()
                ));
            }
        }

        InstKind::DynamicObjectUnbox { value } => {
            expect(
                inst.ty,
                IrType::Nullable(Nullable::ObjectAny),
                position,
                "DynamicObjectUnbox",
                report,
            );
            if let Some(actual) = type_of(value)
                && actual != IrType::Dynamic
            {
                report(format!(
                    "{position}: DynamicObjectUnbox receives {}, expected Dynamic",
                    actual.as_str()
                ));
            }
        }

        InstKind::ObjectConform { object, .. } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ObjectConform",
                report,
            );
            if let Some(actual) = type_of(object)
                && actual != IrType::ObjectAny
            {
                report(format!(
                    "{position}: ObjectConform receiver is {}, expected Object",
                    actual.as_str()
                ));
            }
        }

        InstKind::DynamicPack { value } => {
            expect(inst.ty, IrType::Dynamic, position, "DynamicPack", report);
            if let Some(source) = type_of(value)
                && matches!(
                    source,
                    IrType::Void | IrType::Never | IrType::BorrowedView { .. }
                )
            {
                report(format!(
                    "{position}: DynamicPack cannot box {}",
                    source.as_str()
                ));
            }
        }

        InstKind::DynamicIsType { value, .. } => {
            expect(inst.ty, IrType::Boolean, position, "DynamicIsType", report);
            if let Some(actual) = type_of(value)
                && actual != IrType::Dynamic
            {
                report(format!(
                    "{position}: DynamicIsType receives {}, expected Dynamic",
                    actual.as_str()
                ));
            }
        }

        InstKind::DynamicUnbox { value, tag } => {
            if let Some(actual) = type_of(value)
                && actual != IrType::Dynamic
            {
                report(format!(
                    "{position}: DynamicUnbox receives {}, expected Dynamic",
                    actual.as_str()
                ));
            }
            if inst.ty.dynamic_tag() != Some(*tag) {
                report(format!(
                    "{position}: DynamicUnbox tag {tag:#x} does not match result type {}",
                    inst.ty.as_str()
                ));
            }
        }

        InstKind::ObjectSet { object, key, value } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "ObjectSet",
                report,
            );
            for (operand, expected, label) in [
                (object, IrType::ObjectAny, "receiver"),
                (key, IrType::String, "key"),
                (value, IrType::Dynamic, "value"),
            ] {
                if let Some(actual) = type_of(operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: ObjectSet {label} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }

        InstKind::ObjectGet { object, key } => {
            expect(
                inst.ty,
                IrType::Nullable(Nullable::Dynamic),
                position,
                "ObjectGet",
                report,
            );
            for (operand, expected, label) in [
                (object, IrType::ObjectAny, "receiver"),
                (key, IrType::String, "key"),
            ] {
                if let Some(actual) = type_of(operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: ObjectGet {label} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }

        InstKind::ObjectMerge {
            destination,
            source,
        } => {
            expect(inst.ty, IrType::Void, position, "ObjectMerge", report);
            for (operand, label) in [(destination, "destination"), (source, "source")] {
                if let Some(actual) = type_of(operand)
                    && actual != IrType::ObjectAny
                {
                    report(format!(
                        "{position}: ObjectMerge {label} is {}, expected Object",
                        actual.as_str()
                    ));
                }
            }
        }

        InstKind::ObjectRest {
            object,
            excluded_keys,
        } => {
            expect(inst.ty, IrType::ObjectAny, position, "ObjectRest", report);
            if let Some(actual) = type_of(object)
                && actual != IrType::ObjectAny
            {
                report(format!(
                    "{position}: ObjectRest source is {}, expected Object",
                    actual.as_str()
                ));
            }
            for key in excluded_keys {
                if let Some(actual) = type_of(key)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: ObjectRest exclusion is {}, expected String",
                        actual.as_str()
                    ));
                }
            }
        }

        InstKind::JsonStringifyDynamic { value } => {
            expect(
                inst.ty,
                IrType::Nullable(crate::Nullable::String),
                position,
                "JsonStringifyDynamic",
                report,
            );
            if let Some(actual) = type_of(value)
                && actual != IrType::Dynamic
            {
                report(format!(
                    "{position}: JsonStringifyDynamic value is {}, expected Dynamic",
                    actual.as_str()
                ));
            }
        }
        InstKind::JsonParseDynamic {
            document,
            list_dynamic_id,
        } => {
            expect(
                inst.ty,
                IrType::Nullable(Nullable::Dynamic),
                position,
                "JsonParseDynamic",
                report,
            );
            if let Some(actual) = type_of(document)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: JsonParseDynamic document is {}, expected String",
                    actual.as_str()
                ));
            }
            if module.list_types.get(*list_dynamic_id as usize) != Some(&IrType::Dynamic) {
                report(format!(
                    "{position}: JsonParseDynamic references invalid List<Dynamic> layout {list_dynamic_id}"
                ));
            }
        }

        InstKind::Unary { op, operand } => {
            // Every unary operator preserves its operand's own type — `Neg`
            // and `BitNot` at whatever integer width it is (`Neg` also over
            // any `Float` width, roadmap Phase 3b), `Not` over `Boolean` —
            // so there is one shape to check, not a fixed `Int32` the way
            // this read before the integer-widths migration generalized
            // everything else: an operand at another width or `Float` never
            // exercised this path in a test, so the omission went unnoticed
            // until a `Float` unary `Neg` did.
            if let Some(value) = type_of(operand) {
                let ok = match op {
                    UnaryOp::Neg => matches!(value, IrType::Int(_) | IrType::Float(_)),
                    UnaryOp::BitNot => matches!(value, IrType::Int(_)),
                    UnaryOp::Not => value == IrType::Boolean,
                };
                if !ok {
                    report(format!("{position}: {op:?} applied to {}", value.as_str()));
                }
                expect(inst.ty, value, position, "Unary", report);
            }
        }

        InstKind::Binary { op, left, right } => {
            let (Some(left_ty), Some(right_ty)) = (type_of(left), type_of(right)) else {
                return;
            };
            // A shift's amount is its own, independent integer width — the
            // checker never requires it to match the shifted value's
            // (roadmap Phase 3b, task 4.4); every other operator still
            // requires both sides to agree.
            let shared_object_representation = matches!(op, BinaryOp::Identical)
                && matches!(
                    left_ty,
                    IrType::Object(_) | IrType::Contract(_) | IrType::ObjectAny
                )
                && matches!(
                    right_ty,
                    IrType::Object(_) | IrType::Contract(_) | IrType::ObjectAny
                );
            let shares_type = matches!(op, BinaryOp::Shl | BinaryOp::Shr)
                || left_ty == right_ty
                || shared_object_representation;
            if !shares_type {
                report(format!(
                    "{position}: {op:?} between {} and {}",
                    left_ty.as_str(),
                    right_ty.as_str()
                ));
            }
            expect(inst.ty, op.result_type(left_ty), position, "Binary", report);
        }

        InstKind::CheckedArithmetic {
            op, left, right, ..
        } => {
            let (Some(left_ty), Some(right_ty)) = (type_of(left), type_of(right)) else {
                return;
            };
            if left_ty != right_ty {
                report(format!(
                    "{position}: CheckedArithmetic between {} and {}",
                    left_ty.as_str(),
                    right_ty.as_str()
                ));
            }
            if !matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) {
                report(format!(
                    "{position}: CheckedArithmetic does not cover {op:?}"
                ));
            }
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "CheckedArithmetic",
                report,
            );
        }

        // An `extern "C" fn` (roadmap Phase 4e, design D7) shares this same
        // instruction — no body to check argument count against a
        // `Function`'s own slots, only its declared signature.
        InstKind::Call { callee, args } if module.function(callee).is_none() => {
            match module.externs.iter().find(|e| &e.name == callee) {
                None => report(format!(
                    "{position}: calls `{callee}`, which does not exist"
                )),
                Some(target) => {
                    if args.len() != target.params.len() {
                        report(format!(
                            "{position}: calls `{callee}` with {} argument(s), expected {}",
                            args.len(),
                            target.params.len()
                        ));
                    }
                    if let Some((parameter, result)) = non_safepoint_runtime_leaf_signature(callee)
                    {
                        if target.params.as_slice() != [parameter] || target.return_type != result {
                            report(format!(
                                "{position}: audited runtime leaf `{callee}` must have signature ({}) -> {}",
                                parameter.as_str(),
                                result.as_str()
                            ));
                        }
                        if let Some(argument) = args.first()
                            && let Some(actual) = type_of(argument)
                            && actual != parameter
                        {
                            report(format!(
                                "{position}: argument to audited runtime leaf `{callee}` has type {}, expected {}",
                                actual.as_str(),
                                parameter.as_str()
                            ));
                        }
                    }
                    if callee.starts_with("zirk_rt_atomic_") {
                        for (index, (argument, expected)) in
                            args.iter().zip(&target.params).enumerate()
                        {
                            if let Some(actual) = type_of(argument)
                                && actual != *expected
                            {
                                report(format!(
                                    "{position}: argument {} to extern `{callee}` has type {}, expected {}",
                                    index + 1,
                                    actual.as_str(),
                                    expected.as_str()
                                ));
                            }
                        }
                    }
                    expect(inst.ty, target.return_type, position, "Call", report);
                }
            }
        }
        InstKind::Call { callee, args } => match module.function(callee) {
            None => report(format!(
                "{position}: calls `{callee}`, which does not exist"
            )),
            Some(target) => {
                if args.len() != target.params.len() {
                    report(format!(
                        "{position}: calls `{callee}` with {} argument(s), expected {}",
                        args.len(),
                        target.params.len()
                    ));
                }
                expect(inst.ty, target.return_type, position, "Call", report);
            }
        },

        InstKind::ToString(operand) => {
            expect(inst.ty, IrType::String, position, "ToString", report);
            if let Some(value) = type_of(operand)
                && value == IrType::Void
            {
                report(format!("{position}: ToString applied to Void"));
            }
        }
        InstKind::Float128Format { value, spec } => {
            expect(inst.ty, IrType::String, position, "Float128Format", report);
            if type_of(value) != Some(IrType::Float(crate::FloatWidth::F128)) {
                report(format!("{position}: Float128Format value is not Float128"));
            }
            if type_of(spec) != Some(IrType::String) {
                report(format!("{position}: Float128Format spec is not String"));
            }
        }

        InstKind::Println(operand) | InstKind::PrintlnStderr(operand) => {
            expect(inst.ty, IrType::Void, position, "Println", report);
            // The runtime reads the operand as a string handle: any other type
            // would be interpreted as a pointer to an arbitrary address.
            if let Some(value) = type_of(operand)
                && value != IrType::String
            {
                report(format!(
                    "{position}: Println receives {}, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::Print(operand) | InstKind::PrintStderr(operand) => {
            expect(inst.ty, IrType::Void, position, "Print", report);
            if let Some(value) = type_of(operand)
                && value != IrType::String
            {
                report(format!(
                    "{position}: Print receives {}, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::FlushStdout { enum_id } | InstKind::FlushStderr { enum_id } => {
            expect(inst.ty, IrType::Enum(*enum_id), position, "Flush", report);
        }

        InstKind::StdinReadLineResult { enum_id, .. } | InstKind::StdinReadChar { enum_id, .. } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "StdinRead",
                report,
            );
        }

        InstKind::StdinReadBytes { max, enum_id, .. } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "StdinReadBytes",
                report,
            );
            if let Some(value) = type_of(max)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: StdinReadBytes receives {} for `max`, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::StdinReadExact { count, enum_id, .. } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "StdinReadExact",
                report,
            );
            if let Some(value) = type_of(count)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: StdinReadExact receives {} for `count`, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::EnvironmentGet { name, enum_id, .. } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "EnvironmentGet",
                report,
            );
            if let Some(value) = type_of(name)
                && value != IrType::String
            {
                report(format!(
                    "{position}: EnvironmentGet receives {} for `name`, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::EnvironmentContains { name, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "EnvironmentContains",
                report,
            );
            if let Some(value) = type_of(name)
                && value != IrType::String
            {
                report(format!(
                    "{position}: EnvironmentContains receives {} for `name`, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::EnvironmentListNames { enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "EnvironmentListNames",
                report,
            );
        }

        InstKind::FilesystemCall { binding, args } => {
            if binding.is_empty() || args.len() > 8 {
                report(format!(
                    "{position}: FilesystemCall must name a registered operation and have at most eight arguments"
                ));
            }
            if inst.ty == IrType::Void {
                report(format!(
                    "{position}: FilesystemCall must produce a typed result"
                ));
            }
        }

        InstKind::NetworkTcpConnect { endpoint, enum_id }
        | InstKind::NetworkDnsResolve { endpoint, enum_id }
        | InstKind::NetworkTlsConnect { endpoint, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                if matches!(inst.kind, InstKind::NetworkDnsResolve { .. }) {
                    "NetworkDnsResolve"
                } else {
                    "NetworkTcpConnect"
                },
                report,
            );
            if let Some(value) = type_of(endpoint)
                && value != IrType::String
            {
                report(format!(
                    "{position}: network endpoint instruction receives {} for `endpoint`, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkInterfacesSnapshot { enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkInterfacesSnapshot",
                report,
            );
        }

        InstKind::NetworkUdpConnect { endpoint, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpConnect",
                report,
            );
            if type_of(endpoint) != Some(IrType::String) {
                report(format!(
                    "{position}: NetworkUdpConnect endpoint must be String"
                ));
            }
        }
        InstKind::NetworkUdpSend {
            handle,
            bytes,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpSend",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!("{position}: NetworkUdpSend handle must be UInt64"));
            }
            if !matches!(type_of(bytes), Some(IrType::Array(_))) {
                report(format!(
                    "{position}: NetworkUdpSend payload must be an Array"
                ));
            }
        }
        InstKind::NetworkUdpBind { endpoint, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpBind",
                report,
            );
            if type_of(endpoint) != Some(IrType::String) {
                report(format!(
                    "{position}: NetworkUdpBind endpoint must be String"
                ));
            }
        }
        InstKind::NetworkUdpSendTo {
            handle,
            bytes,
            endpoint,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpSendTo",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64))
                || !matches!(type_of(bytes), Some(IrType::Array(_)))
                || type_of(endpoint) != Some(IrType::String)
            {
                report(format!(
                    "{position}: NetworkUdpSendTo expects (UInt64, Array, String)"
                ));
            }
        }
        InstKind::NetworkUdpSendBroadcast {
            handle,
            bytes,
            endpoint,
            enum_id,
        }
        | InstKind::NetworkUdpSendMulticast {
            handle,
            bytes,
            endpoint,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpSendSpecial",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64))
                || !matches!(type_of(bytes), Some(IrType::Array(_)))
                || type_of(endpoint) != Some(IrType::String)
            {
                report(format!(
                    "{position}: UDP special send expects (UInt64, Array, String)"
                ));
            }
        }
        InstKind::NetworkUdpJoinMulticast {
            handle,
            group,
            enum_id,
        }
        | InstKind::NetworkUdpLeaveMulticast {
            handle,
            group,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpMulticastMembership",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64))
                || type_of(group) != Some(IrType::String)
            {
                report(format!(
                    "{position}: UDP multicast membership expects (UInt64, String)"
                ));
            }
        }
        InstKind::NetworkUdpReceive {
            handle,
            max_bytes,
            enum_id,
            datagram_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpReceive",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkUdpReceive handle must be UInt64"
                ));
            }
            if !matches!(type_of(max_bytes), Some(IrType::Int(_))) {
                report(format!(
                    "{position}: NetworkUdpReceive max_bytes must be an integer"
                ));
            }
            match module.objects.get(*datagram_class_id as usize) {
                Some(datagram)
                    if datagram.fields.len() == 2
                        && matches!(datagram.fields[0].ty, IrType::Array(_))
                        && datagram.fields[1].ty == IrType::String => {}
                Some(_) => report(format!(
                    "{position}: UDPDatagram must contain an Array payload and canonical peer String"
                )),
                None => report(format!(
                    "{position}: NetworkUdpReceive references missing UDPDatagram class layout {datagram_class_id}"
                )),
            }
        }
        InstKind::NetworkUdpClose { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpClose",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!("{position}: NetworkUdpClose handle must be UInt64"));
            }
        }
        InstKind::NetworkUdpLocalAddress { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkUdpLocalAddress",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkUdpLocalAddress handle must be UInt64"
                ));
            }
        }
        InstKind::NetworkUdpIsClosed { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NetworkUdpIsClosed",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkUdpIsClosed handle must be UInt64"
                ));
            }
        }
        InstKind::NetworkLocalConnect {
            endpoint,
            enum_id,
            stream_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkLocalConnect",
                report,
            );
            if type_of(endpoint) != Some(IrType::String) {
                report(format!(
                    "{position}: NetworkLocalConnect endpoint must be String"
                ));
            }
            verify_local_object_result(module, *enum_id, *stream_class_id, position, report);
        }
        InstKind::NetworkLocalBind {
            endpoint,
            remove_stale,
            enum_id,
            listener_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkLocalBind",
                report,
            );
            if type_of(endpoint) != Some(IrType::String)
                || type_of(remove_stale) != Some(IrType::Boolean)
            {
                report(format!(
                    "{position}: NetworkLocalBind expects (String, Boolean)"
                ));
            }
            verify_local_object_result(module, *enum_id, *listener_class_id, position, report);
        }
        InstKind::NetworkLocalAccept {
            handle,
            enum_id,
            stream_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkLocalAccept",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkLocalAccept handle must be UInt64"
                ));
            }
            verify_local_object_result(module, *enum_id, *stream_class_id, position, report);
        }
        InstKind::NetworkLocalRead {
            handle,
            max_bytes,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkLocalRead",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64))
                || !matches!(type_of(max_bytes), Some(IrType::Int(_)))
            {
                report(format!(
                    "{position}: NetworkLocalRead expects (UInt64, integer)"
                ));
            }
        }
        InstKind::NetworkLocalWrite {
            handle,
            bytes,
            enum_id,
            ..
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkLocalWrite",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64))
                || !matches!(type_of(bytes), Some(IrType::Array(_)))
            {
                report(format!(
                    "{position}: NetworkLocalWrite expects (UInt64, Array)"
                ));
            }
        }
        InstKind::NetworkLocalShutdown { handle, enum_id }
        | InstKind::NetworkLocalClose {
            handle, enum_id, ..
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkLocalResource",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkLocalResource handle must be UInt64"
                ));
            }
        }
        InstKind::NetworkLocalIsClosed { handle, .. } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NetworkLocalIsClosed",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkLocalIsClosed handle must be UInt64"
                ));
            }
        }

        InstKind::NetworkIpAddressParse { value, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkIpAddressParse",
                report,
            );
            if let Some(actual) = type_of(value)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: NetworkIpAddressParse receives {}, expected String",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTcpOriginNormalize {
            host,
            port,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpOriginNormalize",
                report,
            );
            if let Some(actual) = type_of(host)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: NetworkTcpOriginNormalize receives {} for `host`, expected String",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(port)
                && actual != IrType::Int(IntWidth::U16)
            {
                report(format!(
                    "{position}: NetworkTcpOriginNormalize receives {} for `port`, expected UInt16",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTcpListen { endpoint, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpListen",
                report,
            );
            if let Some(value) = type_of(endpoint)
                && value != IrType::String
            {
                report(format!(
                    "{position}: NetworkTcpListen receives {} for `endpoint`, expected String",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTcpAccept {
            handle,
            shutdown_control,
            enum_id,
            stream_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpAccept",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpAccept receives {} for `handle`, expected UInt64",
                    value.as_str()
                ));
            }
            if shutdown_control
                .is_some_and(|control| type_of(&control) != Some(IrType::Int(IntWidth::U64)))
            {
                report(format!(
                    "{position}: NetworkTcpAccept shutdown control must be UInt64"
                ));
            }
            if *stream_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: NetworkTcpAccept references missing TCPStream class layout {stream_class_id}"
                ));
            }
        }

        InstKind::NetworkTcpCloseListener { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpCloseListener",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpCloseListener receives {} for `handle`, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTcpListenerIsClosed { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NetworkTcpListenerIsClosed",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpListenerIsClosed receives {} for `handle`, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTcpRead {
            handle,
            max_bytes,
            enum_id,
        }
        | InstKind::NetworkTlsRead {
            handle,
            max_bytes,
            enum_id,
        }
        | InstKind::NetworkTcpWrite {
            handle,
            bytes: max_bytes,
            enum_id,
        }
        | InstKind::NetworkTcpWriteAll {
            handle,
            bytes: max_bytes,
            enum_id,
        }
        | InstKind::NetworkTlsWriteAll {
            handle,
            bytes: max_bytes,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpOperation",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpOperation receives {} for `handle`, expected UInt64",
                    value.as_str()
                ));
            }
            if let Some(value) = type_of(max_bytes)
                && !matches!(value, IrType::Int(IntWidth::U64) | IrType::Array(_))
            {
                report(format!(
                    "{position}: NetworkTcpOperation receives {} for its second operand, expected UInt64 or Array",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTcpShutdown { handle, enum_id }
        | InstKind::NetworkTcpClose { handle, enum_id }
        | InstKind::NetworkTlsShutdown { handle, enum_id }
        | InstKind::NetworkTlsClose { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpResourceOperation",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpResourceOperation receives {} for `handle`, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTcpIsClosed { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NetworkTcpIsClosed",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpIsClosed receives {} for `handle`, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTcpPeerAddress { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTcpPeerAddress",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTcpPeerAddress receives {} for `handle`, expected UInt64",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTlsIsClosed { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NetworkTlsIsClosed",
                report,
            );
            if let Some(value) = type_of(handle)
                && value != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTlsIsClosed receives {}, expected UInt64",
                    value.as_str()
                ));
            }
        }

        InstKind::NetworkTlsListen {
            endpoint,
            certificate_chain_der,
            private_key_der,
            allow_tls12,
            http_alpn: _,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsListen",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (*allow_tls12, IrType::Boolean, "allow_tls12"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: NetworkTlsListen receives {} for `{label}`, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            for (operand, label) in [
                (*certificate_chain_der, "certificate_chain_der"),
                (*private_key_der, "private_key_der"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && !matches!(actual, IrType::Array(_))
                {
                    report(format!(
                        "{position}: NetworkTlsListen receives {} for `{label}`, expected Array",
                        actual.as_str()
                    ));
                }
            }
        }

        InstKind::NetworkTlsListenWithClientAuth {
            endpoint,
            certificate_chain_der,
            private_key_der,
            client_roots_der,
            allow_tls12,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsListenWithClientAuth",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (*allow_tls12, IrType::Boolean, "allow_tls12"),
            ] {
                if type_of(&operand).is_some_and(|actual| actual != expected) {
                    report(format!(
                        "{position}: NetworkTlsListenWithClientAuth `{label}` has incompatible type"
                    ));
                }
            }
            for (operand, label) in [
                (*certificate_chain_der, "certificate_chain_der"),
                (*client_roots_der, "client_roots_der"),
            ] {
                let nested_bytes = matches!(
                    type_of(&operand),
                    Some(IrType::Array(outer))
                        if matches!(module.array_types.get(outer as usize), Some(IrType::Array(inner))
                            if module.array_types.get(*inner as usize) == Some(&IrType::Int(IntWidth::U8)))
                );
                if !nested_bytes {
                    report(format!(
                        "{position}: NetworkTlsListenWithClientAuth `{label}` must be Array<Array<Byte>>"
                    ));
                }
            }
            let private_key_is_bytes = matches!(
                type_of(private_key_der),
                Some(IrType::Array(inner))
                    if module.array_types.get(inner as usize) == Some(&IrType::Int(IntWidth::U8))
            );
            if !private_key_is_bytes {
                report(format!(
                    "{position}: NetworkTlsListenWithClientAuth `private_key_der` must be Array<Byte>"
                ));
            }
        }

        InstKind::NetworkTlsAccept {
            handle,
            shutdown_control,
            enum_id,
            stream_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsAccept",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTlsAccept receives {} for `handle`, expected UInt64",
                    actual.as_str()
                ));
            }
            if shutdown_control
                .is_some_and(|control| type_of(&control) != Some(IrType::Int(IntWidth::U64)))
            {
                report(format!(
                    "{position}: NetworkTlsAccept shutdown control must be UInt64"
                ));
            }
            if *stream_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: NetworkTlsAccept references missing stream object layout {}",
                    stream_class_id
                ));
            }
        }

        InstKind::NetworkTlsCloseListener { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsCloseListener",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTlsCloseListener receives {} for `handle`, expected UInt64",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTlsListenerIsClosed { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NetworkTlsListenerIsClosed",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTlsListenerIsClosed receives {} for `handle`, expected UInt64",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTlsPeerAddress { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsPeerAddress",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTlsPeerAddress receives {} for `handle`, expected UInt64",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTlsNegotiatedAlpn { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsNegotiatedAlpn",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NetworkTlsNegotiatedAlpn receives {} for `handle`, expected UInt64",
                    actual.as_str()
                ));
            }
        }

        InstKind::NetworkTlsPeerCertificateSha256 { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsPeerCertificateSha256",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: NetworkTlsPeerCertificateSha256 handle must be UInt64"
                ));
            }
        }

        InstKind::NetworkTlsConnectWithRoots {
            endpoint,
            roots_der,
            allow_tls12,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsConnectWithRoots",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (*allow_tls12, IrType::Boolean, "allow_tls12"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: NetworkTlsConnectWithRoots receives {} for `{label}`, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            if let Some(actual) = type_of(roots_der)
                && !matches!(actual, IrType::Array(_))
            {
                report(format!(
                    "{position}: NetworkTlsConnectWithRoots receives {} for `roots_der`, expected Array",
                    actual.as_str()
                ));
            }
        }
        InstKind::NetworkTlsConnectWithAlpn {
            endpoint,
            roots_der,
            allow_tls12,
            protocols,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsConnectWithAlpn",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (*allow_tls12, IrType::Boolean, "allow_tls12"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: NetworkTlsConnectWithAlpn receives {} for `{label}`, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            if let Some(actual) = type_of(roots_der)
                && !matches!(actual, IrType::Array(_))
            {
                report(format!(
                    "{position}: NetworkTlsConnectWithAlpn receives {} for `roots_der`, expected Array",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(protocols)
                && !matches!(actual, IrType::Array(_))
            {
                report(format!(
                    "{position}: NetworkTlsConnectWithAlpn receives {} for `protocols`, expected Array<String>",
                    actual.as_str()
                ));
            }
        }
        InstKind::NetworkTlsUpgradeTcp {
            tcp_handle,
            endpoint,
            roots_der,
            allow_tls12,
            protocols,
            enum_id,
            stream_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsUpgradeTcp",
                report,
            );
            for (operand, expected, label) in [
                (*tcp_handle, IrType::Int(IntWidth::U64), "tcp_handle"),
                (*endpoint, IrType::String, "endpoint"),
                (*allow_tls12, IrType::Boolean, "allow_tls12"),
            ] {
                if type_of(&operand).is_some_and(|actual| actual != expected) {
                    report(format!(
                        "{position}: NetworkTlsUpgradeTcp `{label}` has incompatible type"
                    ));
                }
            }
            let nested_bytes = |operand: &Operand| {
                matches!(
                    type_of(operand),
                    Some(IrType::Array(outer))
                        if matches!(module.array_types.get(outer as usize), Some(IrType::Array(inner))
                            if module.array_types.get(*inner as usize) == Some(&IrType::Int(IntWidth::U8)))
                )
            };
            if !nested_bytes(roots_der) {
                report(format!(
                    "{position}: NetworkTlsUpgradeTcp roots_der must be Array<Array<Byte>>"
                ));
            }
            if !matches!(
                type_of(protocols),
                Some(IrType::Array(inner))
                    if module.array_types.get(inner as usize) == Some(&IrType::String)
            ) {
                report(format!(
                    "{position}: NetworkTlsUpgradeTcp protocols must be Array<String>"
                ));
            }
            if *stream_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: NetworkTlsUpgradeTcp references missing TLSStream object layout {}",
                    stream_class_id
                ));
            }
        }
        InstKind::NetworkTlsConnectWithClientIdentity {
            endpoint,
            roots_der,
            certificate_chain_der,
            private_key_der,
            allow_tls12,
            protocols,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "NetworkTlsConnectWithClientIdentity",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (*allow_tls12, IrType::Boolean, "allow_tls12"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: NetworkTlsConnectWithClientIdentity receives {} for `{label}`, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            for (operand, label) in [
                (*roots_der, "roots_der"),
                (*certificate_chain_der, "certificate_chain_der"),
            ] {
                let is_certificates = matches!(
                    type_of(&operand),
                    Some(IrType::Array(outer))
                        if matches!(module.array_types.get(outer as usize), Some(IrType::Array(inner))
                            if module.array_types.get(*inner as usize) == Some(&IrType::Int(IntWidth::U8)))
                );
                if !is_certificates {
                    report(format!(
                        "{position}: NetworkTlsConnectWithClientIdentity `{label}` must be Array<Array<Byte>>"
                    ));
                }
            }
            if !matches!(
                type_of(private_key_der),
                Some(IrType::Array(inner))
                    if module.array_types.get(inner as usize) == Some(&IrType::Int(IntWidth::U8))
            ) {
                report(format!(
                    "{position}: NetworkTlsConnectWithClientIdentity private_key_der must be Array<Byte>"
                ));
            }
            if !matches!(
                type_of(protocols),
                Some(IrType::Array(inner))
                    if module.array_types.get(inner as usize) == Some(&IrType::String)
            ) {
                report(format!(
                    "{position}: NetworkTlsConnectWithClientIdentity protocols must be Array<String>"
                ));
            }
        }

        InstKind::HttpGet {
            url,
            cached: _,
            enum_id,
            response_class_id,
        } => {
            expect(inst.ty, IrType::Enum(*enum_id), position, "HttpGet", report);
            if let Some(value) = type_of(url)
                && value != IrType::String
            {
                report(format!(
                    "{position}: HttpGet receives {} for `url`, expected String",
                    value.as_str()
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpGet references missing response object layout {}",
                    response_class_id
                ));
            }
        }
        InstKind::HttpRequest {
            method,
            url,
            body,
            headers,
            validate_json,
            cached: _,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpRequest",
                report,
            );
            for (label, operand) in [("method", method), ("url", url)] {
                if let Some(actual) = type_of(operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpRequest receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
            for (label, operand) in [("body", body), ("headers", headers)] {
                let is_byte_array = match type_of(operand) {
                    Some(IrType::Array(array_id)) => {
                        module.array_types.get(array_id as usize).copied()
                            == Some(IrType::Int(IntWidth::U8))
                    }
                    _ => false,
                };
                if !is_byte_array {
                    report(format!(
                        "{position}: HttpRequest receives a non-byte array for `{label}`"
                    ));
                }
            }
            if let Some(actual) = type_of(validate_json)
                && actual != IrType::Boolean
            {
                report(format!(
                    "{position}: HttpRequest receives {} for `validate_json`, expected Boolean",
                    actual.as_str()
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpRequest references missing response object layout {}",
                    response_class_id
                ));
            }
        }
        InstKind::HttpClientCreate {
            max_origins,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientCreate",
                report,
            );
            if let Some(actual) = type_of(max_origins)
                && actual != IrType::Int(IntWidth::I32)
            {
                report(format!(
                    "{position}: HttpClientCreate max_origins must be Int32"
                ));
            }
        }
        InstKind::HttpClientCreateWithOptions {
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientCreateWithOptions",
                report,
            );
            for (operand, expected, label) in [
                (max_origins, IrType::Int(IntWidth::I32), "max_origins"),
                (
                    connect_timeout_ms,
                    IrType::Int(IntWidth::I64),
                    "connect_timeout_ms",
                ),
                (
                    response_headers_timeout_ms,
                    IrType::Int(IntWidth::I64),
                    "response_headers_timeout_ms",
                ),
                (
                    response_idle_timeout_ms,
                    IrType::Int(IntWidth::I64),
                    "response_idle_timeout_ms",
                ),
                (protocol_code, IrType::Int(IntWidth::I32), "protocol_code"),
            ] {
                if let Some(actual) = type_of(operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: HttpClientCreateWithOptions {label} has type {actual:?}, expected {expected:?}"
                    ));
                }
            }
        }
        InstKind::HttpClientCreateWithTrustRoots {
            roots,
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientCreateWithTrustRoots",
                report,
            );
            if !matches!(type_of(roots), Some(IrType::Array(_))) {
                report(format!(
                    "{position}: HttpClientCreateWithTrustRoots roots must be an array"
                ));
            }
            if let Some(actual) = type_of(max_origins)
                && actual != IrType::Int(IntWidth::I32)
            {
                report(format!(
                    "{position}: HttpClientCreateWithTrustRoots max_origins must be Int32"
                ));
            }
            for (operand, label) in [
                (connect_timeout_ms, "connect_timeout_ms"),
                (response_headers_timeout_ms, "response_headers_timeout_ms"),
                (response_idle_timeout_ms, "response_idle_timeout_ms"),
            ] {
                if type_of(operand) != Some(IrType::Int(IntWidth::I64)) {
                    report(format!(
                        "{position}: HttpClientCreateWithTrustRoots {label} must be UInt64"
                    ));
                }
            }
            if type_of(protocol_code) != Some(IrType::Int(IntWidth::I32)) {
                report(format!(
                    "{position}: HttpClientCreateWithTrustRoots protocol_code must be Int32"
                ));
            }
        }
        InstKind::HttpClientCreateWithProxy {
            proxy_url,
            proxy_credentials,
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientCreateWithProxy",
                report,
            );
            if let Some(actual) = type_of(proxy_url)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpClientCreateWithProxy proxy_url must be String"
                ));
            }
            if let Some(actual) = type_of(proxy_credentials)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpClientCreateWithProxy proxy_credentials must be String"
                ));
            }
            if let Some(actual) = type_of(max_origins)
                && actual != IrType::Int(IntWidth::I32)
            {
                report(format!(
                    "{position}: HttpClientCreateWithProxy max_origins must be Int32"
                ));
            }
            for (operand, label) in [
                (connect_timeout_ms, "connect_timeout_ms"),
                (response_headers_timeout_ms, "response_headers_timeout_ms"),
                (response_idle_timeout_ms, "response_idle_timeout_ms"),
            ] {
                if type_of(operand) != Some(IrType::Int(IntWidth::I64)) {
                    report(format!(
                        "{position}: HttpClientCreateWithProxy {label} must be UInt64"
                    ));
                }
            }
            if type_of(protocol_code) != Some(IrType::Int(IntWidth::I32)) {
                report(format!(
                    "{position}: HttpClientCreateWithProxy protocol_code must be Int32"
                ));
            }
        }
        InstKind::HttpClientCreateWithProxyAndTrustRoots {
            proxy_url,
            proxy_credentials,
            roots,
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientCreateWithProxyAndTrustRoots",
                report,
            );
            if type_of(proxy_url) != Some(IrType::String)
                || type_of(proxy_credentials) != Some(IrType::String)
            {
                report(format!(
                    "{position}: HttpClientCreateWithProxyAndTrustRoots proxy values must be String"
                ));
            }
            if !matches!(type_of(roots), Some(IrType::Array(_))) {
                report(format!(
                    "{position}: HttpClientCreateWithProxyAndTrustRoots roots must be an array"
                ));
            }
            if type_of(max_origins) != Some(IrType::Int(IntWidth::I32)) {
                report(format!(
                    "{position}: HttpClientCreateWithProxyAndTrustRoots max_origins must be Int32"
                ));
            }
            for (operand, label) in [
                (connect_timeout_ms, "connect_timeout_ms"),
                (response_headers_timeout_ms, "response_headers_timeout_ms"),
                (response_idle_timeout_ms, "response_idle_timeout_ms"),
            ] {
                if type_of(operand) != Some(IrType::Int(IntWidth::I64)) {
                    report(format!(
                        "{position}: HttpClientCreateWithProxyAndTrustRoots {label} must be UInt64"
                    ));
                }
            }
            if type_of(protocol_code) != Some(IrType::Int(IntWidth::I32)) {
                report(format!(
                    "{position}: HttpClientCreateWithProxyAndTrustRoots protocol_code must be Int32"
                ));
            }
        }
        InstKind::HttpClientRequest {
            handle,
            method,
            url,
            body,
            headers,
            validate_json,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientRequest",
                report,
            );
            if type_of(handle).is_some_and(|ty| ty != IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: HttpClientRequest handle must be UInt64"
                ));
            }
            for (label, operand) in [("method", method), ("url", url)] {
                if type_of(operand).is_some_and(|ty| ty != IrType::String) {
                    report(format!(
                        "{position}: HttpClientRequest {label} must be String"
                    ));
                }
            }
            for (label, operand) in [("body", body), ("headers", headers)] {
                let bytes = matches!(type_of(operand), Some(IrType::Array(id)) if module.array_types.get(id as usize) == Some(&IrType::Int(IntWidth::U8)));
                if !bytes {
                    report(format!(
                        "{position}: HttpClientRequest {label} must be Array<Byte>"
                    ));
                }
            }
            if type_of(validate_json).is_some_and(|ty| ty != IrType::Boolean) {
                report(format!(
                    "{position}: HttpClientRequest validate_json must be Boolean"
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpClientRequest references missing HTTPResponse layout"
                ));
            }
        }
        InstKind::HttpClientRequestStream {
            handle,
            method,
            url,
            headers,
            callback,
            callback_layout,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientRequestStream",
                report,
            );
            if type_of(handle).is_some_and(|ty| ty != IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: HttpClientRequestStream handle must be UInt64"
                ));
            }
            for (label, operand) in [("method", method), ("url", url)] {
                if type_of(operand).is_some_and(|ty| ty != IrType::String) {
                    report(format!(
                        "{position}: HttpClientRequestStream {label} must be String"
                    ));
                }
            }
            let bytes = matches!(type_of(headers), Some(IrType::Array(id)) if module.array_types.get(id as usize) == Some(&IrType::Int(IntWidth::U8)));
            if !bytes {
                report(format!(
                    "{position}: HttpClientRequestStream headers must be Array<Byte>"
                ));
            }
            let callback_ok = module.closures.get(*callback_layout as usize).is_some_and(|layout| {
                layout.params == [IrType::Int(IntWidth::U64)]
                    && matches!(layout.returns, IrType::Array(id) if module.array_types.get(id as usize) == Some(&IrType::Int(IntWidth::U8)))
            });
            if !callback_ok {
                report(format!(
                    "{position}: HttpClientRequestStream callback must take UInt64 and return Array<Byte>"
                ));
            }
            if type_of(callback).is_some_and(|ty| ty != IrType::Callable(*callback_layout)) {
                report(format!(
                    "{position}: HttpClientRequestStream callback has an invalid callable layout"
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpClientRequestStream references missing HTTPResponse layout"
                ));
            }
        }
        InstKind::HttpClientAction {
            handle, enum_id, ..
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpClientAction",
                report,
            );
            if type_of(handle).is_some_and(|ty| ty != IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: HttpClientAction handle must be UInt64"
                ));
            }
        }
        InstKind::HttpWebSocketConnect {
            url,
            origin,
            subprotocols,
            http3: _,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpWebSocketConnect",
                report,
            );
            if type_of(url) != Some(IrType::String) || type_of(origin) != Some(IrType::String) {
                report(format!(
                    "{position}: HttpWebSocketConnect URL and Origin must be String"
                ));
            }
            if !matches!(type_of(subprotocols), Some(IrType::List(id)) if module.list_types.get(id as usize) == Some(&IrType::String))
            {
                report(format!(
                    "{position}: HttpWebSocketConnect subprotocols must be List<String>"
                ));
            }
        }
        InstKind::HttpWebSocketConnectWithTrustRoots {
            url,
            origin,
            subprotocols,
            roots_der,
            allow_tls12,
            http3: _,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpWebSocketConnectWithTrustRoots",
                report,
            );
            if type_of(url) != Some(IrType::String) || type_of(origin) != Some(IrType::String) {
                report(format!(
                    "{position}: HttpWebSocketConnectWithTrustRoots URL and Origin must be String"
                ));
            }
            if !matches!(type_of(subprotocols), Some(IrType::List(id)) if module.list_types.get(id as usize) == Some(&IrType::String))
            {
                report(format!(
                    "{position}: HttpWebSocketConnectWithTrustRoots subprotocols must be List<String>"
                ));
            }
            if !matches!(type_of(roots_der), Some(IrType::Array(_))) {
                report(format!(
                    "{position}: HttpWebSocketConnectWithTrustRoots roots must be an array"
                ));
            }
            if type_of(allow_tls12) != Some(IrType::Boolean) {
                report(format!(
                    "{position}: HttpWebSocketConnectWithTrustRoots allow_tls12 must be Boolean"
                ));
            }
        }
        InstKind::HttpWebSocketSend {
            handle,
            record,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpWebSocketSend",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: HttpWebSocketSend handle must be UInt64"
                ));
            }
            if !matches!(type_of(record), Some(IrType::Array(id)) if module.array_types.get(id as usize) == Some(&IrType::Int(IntWidth::U8)))
            {
                report(format!(
                    "{position}: HttpWebSocketSend record must be Array<Byte>"
                ));
            }
        }
        InstKind::HttpWebSocketReceive { handle, enum_id }
        | InstKind::HttpWebSocketClose { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "WebSocket action",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: WebSocket action handle must be UInt64"
                ));
            }
        }
        InstKind::HttpWebSocketCloseWith {
            handle,
            code,
            reason,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "WebSocket close_with",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: WebSocket close_with handle must be UInt64"
                ));
            }
            if type_of(code) != Some(IrType::Int(IntWidth::U16)) {
                report(format!(
                    "{position}: WebSocket close_with code must be UInt16"
                ));
            }
            if type_of(reason) != Some(IrType::String) {
                report(format!(
                    "{position}: WebSocket close_with reason must be String"
                ));
            }
        }
        InstKind::HttpRequestSecret {
            method,
            url,
            body,
            headers,
            header_name,
            secret_value,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpRequestSecret",
                report,
            );
            for (label, operand) in [
                ("method", method),
                ("url", url),
                ("header_name", header_name),
                ("secret_value", secret_value),
            ] {
                if let Some(actual) = type_of(operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpRequestSecret receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
            for (label, operand) in [("body", body), ("headers", headers)] {
                let is_byte_array = match type_of(operand) {
                    Some(IrType::Array(array_id)) => {
                        module.array_types.get(array_id as usize).copied()
                            == Some(IrType::Int(IntWidth::U8))
                    }
                    _ => false,
                };
                if !is_byte_array {
                    report(format!(
                        "{position}: HttpRequestSecret receives a non-byte array for `{label}`"
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpRequestSecret references missing response object layout {}",
                    response_class_id
                ));
            }
        }
        InstKind::HttpHead {
            url,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpHead",
                report,
            );
            if let Some(value) = type_of(url)
                && value != IrType::String
            {
                report(format!(
                    "{position}: HttpHead receives {} for `url`, expected String",
                    value.as_str()
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpHead references missing response object layout {}",
                    response_class_id
                ));
            }
        }
        InstKind::HttpQuery {
            url,
            query,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpQuery",
                report,
            );
            for (operand, label) in [(*url, "url"), (*query, "query")] {
                if let Some(actual) = type_of(&operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpQuery receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpQuery references missing response object layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpQueryWithContentType {
            url,
            query,
            content_type,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpQueryWithContentType",
                report,
            );
            for (operand, label) in [
                (*url, "url"),
                (*query, "query"),
                (*content_type, "content_type"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpQueryWithContentType receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpQueryWithContentType references missing response object layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpPost {
            url,
            body,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpPost",
                report,
            );
            for (operand, label) in [(*url, "url"), (*body, "body")] {
                if let Some(actual) = type_of(&operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpPost receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpPost references missing response object layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpPostJson {
            url,
            body,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpPostJson",
                report,
            );
            for (operand, label) in [(*url, "url"), (*body, "body")] {
                if let Some(actual) = type_of(&operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpPostJson receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpPostJson references missing response object layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpPostBytes {
            url,
            body,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpPostBytes",
                report,
            );
            if let Some(actual) = type_of(url)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpPostBytes receives {} for `url`, expected String",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(body) {
                let is_byte_array = match actual {
                    IrType::Array(array_id) => {
                        module.array_types.get(array_id as usize).copied()
                            == Some(IrType::Int(IntWidth::U8))
                    }
                    _ => false,
                };
                if !is_byte_array {
                    report(format!(
                        "{position}: HttpPostBytes receives {} for `body`, expected Array<Byte>",
                        actual.as_str()
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpPostBytes references missing response object layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpGetStream {
            url,
            headers,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpGetStream",
                report,
            );
            if let Some(actual) = type_of(url)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpGetStream receives {}, expected String URL",
                    actual.as_str()
                ));
            }
            if !matches!(type_of(headers), Some(IrType::Array(array_id))
                if module.array_types.get(array_id as usize).copied()
                    == Some(IrType::Int(IntWidth::U8)))
            {
                report(format!(
                    "{position}: HttpGetStream expected encoded headers as Array<Byte>"
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpGetStream references missing stream-response object layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpResponseToStream {
            status,
            body,
            headers,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpResponseToStream",
                report,
            );
            if let Some(actual) = type_of(status)
                && actual != IrType::Int(IntWidth::I32)
            {
                report(format!(
                    "{position}: HttpResponseToStream receives {} for status, expected Int32",
                    actual.as_str()
                ));
            }
            for (operand, label) in [(body, "body"), (headers, "headers")] {
                let is_byte_array = matches!(type_of(operand), Some(IrType::Array(array_id))
                    if module.array_types.get(array_id as usize).copied()
                        == Some(IrType::Int(IntWidth::U8)));
                if !is_byte_array {
                    report(format!(
                        "{position}: HttpResponseToStream {label} must be Array<Byte>"
                    ));
                }
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpResponseToStream references missing stream-response layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpRequestStream {
            method,
            url,
            headers,
            callback,
            callback_layout,
            result_callback,
            enum_id,
            response_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpRequestStream",
                report,
            );
            if let Some(actual) = type_of(method)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpRequestStream receives {} for method, expected String",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(url)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpRequestStream receives {} for URL, expected String",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(headers)
                && !matches!(actual, IrType::Array(_))
            {
                report(format!(
                    "{position}: HttpRequestStream receives {} for headers, expected Array<Byte>",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(headers)
                && let IrType::Array(array_id) = actual
                && module.array_types.get(array_id as usize).copied()
                    != Some(IrType::Int(IntWidth::U8))
            {
                report(format!(
                    "{position}: HttpRequestStream headers must be Array<Byte>"
                ));
            }
            let callback_ok =
                module
                    .closures
                    .get(*callback_layout as usize)
                    .is_some_and(|layout| {
                        let returns_bytes = |ty: IrType| {
                            matches!(ty, IrType::Array(array_id)
                            if module.array_types.get(array_id as usize).copied()
                                == Some(IrType::Int(IntWidth::U8)))
                        };
                        let returns_result_bytes = |ty: IrType| {
                            let IrType::Enum(result_id) = ty else {
                                return false;
                            };
                            let Some(result) = module.enums.get(result_id as usize) else {
                                return false;
                            };
                            result.variants.len() == 2
                                && result.variants[0].first().is_some_and(|field| {
                                    result
                                        .fields
                                        .get(*field as usize)
                                        .is_some_and(|field| returns_bytes(field.ty))
                                })
                                && result.variants[1].first().is_some_and(|field| {
                                    result
                                        .fields
                                        .get(*field as usize)
                                        .is_some_and(|field| field.ty == IrType::Int(IntWidth::I32))
                                })
                        };
                        layout.params == [IrType::Int(IntWidth::U64)]
                            && if *result_callback {
                                returns_result_bytes(layout.returns)
                            } else {
                                returns_bytes(layout.returns)
                            }
                    });
            if !callback_ok {
                let contract = if *result_callback {
                    "UInt64 and return Result<Array<Byte>, HTTPStreamProducerError>"
                } else {
                    "UInt64 and return Array<Byte>"
                };
                report(format!(
                    "{position}: HttpRequestStream callback must take {contract}"
                ));
            }
            if let Some(actual) = type_of(callback)
                && actual != IrType::Callable(*callback_layout)
            {
                report(format!(
                    "{position}: HttpRequestStream receives {} for callback, expected callable layout {callback_layout}",
                    actual.as_str()
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpRequestStream references missing stream-response layout {response_class_id}"
                ));
            }
        }
        InstKind::HttpRegisterServerResponseProducer {
            callback,
            callback_layout,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "HttpRegisterServerResponseProducer",
                report,
            );
            let callback_ok =
                module
                    .closures
                    .get(*callback_layout as usize)
                    .is_some_and(|layout| {
                        layout.params == [IrType::Int(IntWidth::U64)]
                            && matches!(layout.returns, IrType::Array(array_id)
                            if module.array_types.get(array_id as usize).copied()
                                == Some(IrType::Int(IntWidth::U8)))
                    });
            if !callback_ok {
                report(format!(
                    "{position}: HttpRegisterServerResponseProducer callback must take UInt64 and return Array<Byte>"
                ));
            }
            if let Some(actual) = type_of(callback)
                && actual != IrType::Callable(*callback_layout)
            {
                report(format!(
                    "{position}: HttpRegisterServerResponseProducer receives {} for callback, expected callable layout {callback_layout}",
                    actual.as_str()
                ));
            }
        }
        InstKind::HttpResponseReadChunk {
            handle,
            max_bytes,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpResponseReadChunk",
                report,
            );
            for (operand, label) in [(*handle, "handle"), (*max_bytes, "max_bytes")] {
                if let Some(actual) = type_of(&operand)
                    && actual != IrType::Int(IntWidth::U64)
                {
                    report(format!(
                        "{position}: HttpResponseReadChunk receives {} for `{label}`, expected UInt64",
                        actual.as_str()
                    ));
                }
            }
        }
        InstKind::HttpResponseReadAll { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpResponseReadAll",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: HttpResponseReadAll receives {}, expected UInt64 handle",
                    actual.as_str()
                ));
            }
        }
        InstKind::HttpResponseReadSseEvent { handle, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpResponseReadSseEvent",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: HttpResponseReadSseEvent receives {}, expected UInt64 handle",
                    actual.as_str()
                ));
            }
            let success_is_list_string = module
                .enums
                .get(*enum_id as usize)
                .and_then(|layout| layout.variants.first())
                .and_then(|success| success.first())
                .and_then(|field| {
                    module
                        .enums
                        .get(*enum_id as usize)?
                        .fields
                        .get(*field as usize)
                })
                .is_some_and(|field| match field.ty {
                    IrType::List(list_id) => {
                        module.list_types.get(list_id as usize).copied() == Some(IrType::String)
                    }
                    _ => false,
                });
            if !success_is_list_string {
                report(format!(
                    "{position}: HttpResponseReadSseEvent success must contain List<String>"
                ));
            }
        }
        InstKind::HttpResponseSseHeartbeatCount { handle } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "HttpResponseSseHeartbeatCount",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: HttpResponseSseHeartbeatCount receives {}, expected UInt64 handle",
                    actual.as_str()
                ));
            }
        }
        InstKind::HttpResponseStreamClose { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "HttpResponseStreamClose",
                report,
            );
            if let Some(actual) = type_of(handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: HttpResponseStreamClose receives {}, expected UInt64 handle",
                    actual.as_str()
                ));
            }
        }

        InstKind::HttpUrlNormalize { value, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpUrlNormalize",
                report,
            );
            if let Some(actual) = type_of(value)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpUrlNormalize receives {}, expected String",
                    actual.as_str()
                ));
            }
        }

        InstKind::HttpUrlResolve {
            base,
            reference,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpUrlResolve",
                report,
            );
            for (operand, label) in [(base, "base"), (reference, "reference")] {
                if let Some(actual) = type_of(operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpUrlResolve receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
        }

        InstKind::HttpResponseText { body, enum_id } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpResponseText",
                report,
            );
            let is_byte_array = match type_of(body) {
                Some(IrType::Array(id)) => {
                    module.array_types.get(id as usize).copied() == Some(IrType::Int(IntWidth::U8))
                }
                _ => false,
            };
            if !is_byte_array {
                report(format!(
                    "{position}: HttpResponseText receives a non-byte array, expected Array<Byte>"
                ));
            }
        }

        InstKind::HttpResponseHeaderValues {
            headers,
            name,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpResponseHeaderValues",
                report,
            );
            let headers_are_bytes = match type_of(headers) {
                Some(IrType::Array(id)) => {
                    module.array_types.get(id as usize).copied() == Some(IrType::Int(IntWidth::U8))
                }
                _ => false,
            };
            if !headers_are_bytes {
                report(format!(
                    "{position}: HttpResponseHeaderValues receives a non-byte array for encoded headers"
                ));
            }
            if let Some(actual) = type_of(name)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpResponseHeaderValues receives {} for `name`, expected String",
                    actual.as_str()
                ));
            }
        }

        InstKind::HttpRouteMatch { pattern, path } => {
            if !matches!(inst.ty, IrType::List(_)) {
                report(format!(
                    "{position}: HttpRouteMatch returns {}, expected List<String>",
                    inst.ty.as_str()
                ));
            } else if let IrType::List(list_id) = inst.ty
                && module.list_types.get(list_id as usize).copied() != Some(IrType::String)
            {
                report(format!(
                    "{position}: HttpRouteMatch returns List<{}>, expected List<String>",
                    module
                        .list_types
                        .get(list_id as usize)
                        .map(|list| list.as_str())
                        .unwrap_or("<missing>"),
                ));
            }
            for (label, operand) in [("pattern", pattern), ("path", path)] {
                if let Some(actual) = type_of(operand)
                    && actual != IrType::String
                {
                    report(format!(
                        "{position}: HttpRouteMatch receives {} for `{label}`, expected String",
                        actual.as_str()
                    ));
                }
            }
        }

        InstKind::HttpFormParse {
            body,
            content_type,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpFormParse",
                report,
            );
            let is_bytes = match type_of(body) {
                Some(IrType::Array(id)) => {
                    module.array_types.get(id as usize).copied() == Some(IrType::Int(IntWidth::U8))
                }
                _ => false,
            };
            if !is_bytes {
                report(format!(
                    "{position}: HttpFormParse body must be Array<Byte>"
                ));
            }
            if let Some(actual) = type_of(content_type)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpFormParse content type must be String, received {}",
                    actual.as_str()
                ));
            }
            let result_is_list_string = module
                .enums
                .get(*enum_id as usize)
                .and_then(|layout| layout.variants.first())
                .and_then(|success| success.first())
                .and_then(|field| {
                    module
                        .enums
                        .get(*enum_id as usize)?
                        .fields
                        .get(*field as usize)
                })
                .is_some_and(|field| match field.ty {
                    IrType::List(list_id) => {
                        module.list_types.get(list_id as usize).copied() == Some(IrType::String)
                    }
                    _ => false,
                });
            if !result_is_list_string {
                report(format!(
                    "{position}: HttpFormParse success must contain List<String>"
                ));
            }
        }
        InstKind::HttpMultipartParse {
            body,
            content_type,
            enum_id,
            data_class_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpMultipartParse",
                report,
            );
            if !matches!(type_of(body), Some(IrType::Array(id)) if module.array_types.get(id as usize).copied() == Some(IrType::Int(IntWidth::U8)))
            {
                report(format!(
                    "{position}: HttpMultipartParse body must be Array<Byte>"
                ));
            }
            if let Some(actual) = type_of(content_type)
                && actual != IrType::String
            {
                report(format!(
                    "{position}: HttpMultipartParse content type must be String, received {}",
                    actual.as_str()
                ));
            }
            let payload_class = module
                .enums
                .get(*enum_id as usize)
                .and_then(|layout| layout.variants.first())
                .and_then(|success| success.first())
                .and_then(|field| {
                    module
                        .enums
                        .get(*enum_id as usize)?
                        .fields
                        .get(*field as usize)
                })
                .is_some_and(|field| field.ty == IrType::Object(*data_class_id));
            if !payload_class {
                report(format!(
                    "{position}: HttpMultipartParse success must contain its HTTPMultipartData object"
                ));
            }
            let class_has_expected_fields = module
                .objects
                .get(*data_class_id as usize)
                .is_some_and(|layout| {
                    if layout.fields.len() != 7 {
                        return false;
                    }
                    let field_types = layout
                        .fields
                        .iter()
                        .map(|field| field.ty)
                        .collect::<Vec<_>>();
                    let list_item = |ty: IrType| match ty {
                        IrType::List(id) => module.list_types.get(id as usize).copied(),
                        _ => None,
                    };
                    let names_ok = list_item(field_types[0]) == Some(IrType::String);
                    let filenames_ok = list_item(field_types[1]) == Some(IrType::String);
                    let filename_presence_ok = list_item(field_types[2]) == Some(IrType::Boolean);
                    let content_types_ok = list_item(field_types[3]) == Some(IrType::String);
                    let content_type_presence_ok =
                        list_item(field_types[4]) == Some(IrType::Boolean);
                    let bodies_ok = match list_item(field_types[5]) {
                        Some(IrType::Array(array_id)) => {
                            module.array_types.get(array_id as usize).copied()
                                == Some(IrType::Int(IntWidth::U8))
                        }
                        _ => false,
                    };
                    let upload_handles_ok =
                        list_item(field_types[6]) == Some(IrType::Int(IntWidth::U64));
                    names_ok
                        && filenames_ok
                        && filename_presence_ok
                        && content_types_ok
                        && content_type_presence_ok
                        && bodies_ok
                        && upload_handles_ok
                });
            if !class_has_expected_fields {
                report(format!(
                    "{position}: HttpMultipartParse data class must have the seven bundled parallel lists"
                ));
            }
        }
        InstKind::HttpUploadRead {
            handle,
            offset,
            max_bytes,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpUploadRead",
                report,
            );
            for (label, operand) in [
                ("handle", handle),
                ("offset", offset),
                ("max_bytes", max_bytes),
            ] {
                if type_of(operand) != Some(IrType::Int(IntWidth::U64)) {
                    report(format!("{position}: HttpUploadRead {label} must be UInt64"));
                }
            }
            if module
                .enums
                .get(*enum_id as usize)
                .is_none_or(|layout| layout.name != "HTTPStreamChunk")
            {
                report(format!(
                    "{position}: HttpUploadRead must return HTTPStreamChunk"
                ));
            }
        }
        InstKind::HttpUploadClose { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "HttpUploadClose",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!("{position}: HttpUploadClose handle must be UInt64"));
            }
        }
        InstKind::HttpServerControlCreate => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "HttpServerControlCreate",
                report,
            );
        }
        InstKind::HttpServerControlRequestShutdown { handle } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "HttpServerControlRequestShutdown",
                report,
            );
            if type_of(handle) != Some(IrType::Int(IntWidth::U64)) {
                report(format!(
                    "{position}: HttpServerControlRequestShutdown handle must be UInt64"
                ));
            }
        }

        InstKind::HttpServerServeOnce {
            stream_handle,
            secure: _,
            max_requests,
            require_http2: _,
            callback,
            callback_layout,
            request_class_id,
            response_class_id,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpServerServeOnce",
                report,
            );
            if let Some(actual) = type_of(stream_handle)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: HttpServerServeOnce receives {} for stream handle, expected UInt64",
                    actual.as_str()
                ));
            }
            if let Some(actual) = type_of(max_requests)
                && actual != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: HttpServerServeOnce receives {} for max requests, expected UInt64",
                    actual.as_str()
                ));
            }
            if *request_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpServerServeOnce references missing request object layout {request_class_id}"
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpServerServeOnce references missing response object layout {response_class_id}"
                ));
            }
            let callback_ok =
                module
                    .closures
                    .get(*callback_layout as usize)
                    .is_some_and(|layout| {
                        layout.params == vec![IrType::Object(*request_class_id)]
                            && layout.returns == IrType::Object(*response_class_id)
                    });
            if !callback_ok {
                report(format!(
                    "{position}: HttpServerServeOnce callback layout {callback_layout} must accept the request object and return the response object"
                ));
            }
            if let Some(actual) = type_of(callback)
                && actual != IrType::Callable(*callback_layout)
            {
                report(format!(
                    "{position}: HttpServerServeOnce receives {} for callback, expected callable layout {callback_layout}",
                    actual.as_str()
                ));
            }
        }

        InstKind::HttpServerServeHttp3 {
            endpoint,
            certificate_chain_der,
            private_key_der,
            max_requests,
            max_connections,
            control_handle,
            drain_timeout_ms,
            max_handshakes,
            max_handler_tasks,
            max_streams_per_connection,
            max_queued_response_bytes,
            callback,
            callback_layout,
            request_class_id,
            response_class_id,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpServerServeHttp3",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (
                    *max_requests,
                    IrType::Int(IntWidth::U64),
                    "maximum requests",
                ),
                (
                    *max_connections,
                    IrType::Int(IntWidth::U64),
                    "maximum connections",
                ),
                (
                    *control_handle,
                    IrType::Int(IntWidth::U64),
                    "shutdown control handle",
                ),
                (
                    *drain_timeout_ms,
                    IrType::Int(IntWidth::U64),
                    "drain timeout",
                ),
                (
                    *max_handshakes,
                    IrType::Int(IntWidth::U64),
                    "maximum concurrent handshakes",
                ),
                (
                    *max_handler_tasks,
                    IrType::Int(IntWidth::U64),
                    "maximum handler tasks",
                ),
                (
                    *max_streams_per_connection,
                    IrType::Int(IntWidth::U64),
                    "maximum streams per connection",
                ),
                (
                    *max_queued_response_bytes,
                    IrType::Int(IntWidth::U64),
                    "maximum queued response bytes",
                ),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: HttpServerServeHttp3 {label} has type {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            for (operand, label) in [
                (*certificate_chain_der, "certificate chain"),
                (*private_key_der, "private key"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && !matches!(actual, IrType::Array(_))
                {
                    report(format!(
                        "{position}: HttpServerServeHttp3 {label} must be an array, got {}",
                        actual.as_str()
                    ));
                }
            }
            if let Some(layout) = module.enums.get(*enum_id as usize) {
                let success_payload = layout
                    .variants
                    .first()
                    .and_then(|variant| variant.first())
                    .and_then(|field| layout.fields.get(*field as usize));
                if success_payload.map(|field| field.ty) != Some(IrType::Int(IntWidth::U64)) {
                    report(format!(
                        "{position}: HttpServerServeHttp3 success payload must be UInt64"
                    ));
                }
            }
            if *request_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpServerServeHttp3 references missing request object layout {request_class_id}"
                ));
            }
            if *response_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpServerServeHttp3 references missing response object layout {response_class_id}"
                ));
            }
            let callback_ok =
                module
                    .closures
                    .get(*callback_layout as usize)
                    .is_some_and(|layout| {
                        layout.params == vec![IrType::Object(*request_class_id)]
                            && layout.returns == IrType::Object(*response_class_id)
                    });
            if !callback_ok {
                report(format!(
                    "{position}: HttpServerServeHttp3 callback layout {callback_layout} must accept the request object and return the response object"
                ));
            }
            if let Some(actual) = type_of(callback)
                && actual != IrType::Callable(*callback_layout)
            {
                report(format!(
                    "{position}: HttpServerServeHttp3 receives {} for callback, expected callable layout {callback_layout}",
                    actual.as_str()
                ));
            }
        }

        InstKind::HttpServerWebSocketServeOnce {
            stream_handle,
            secure: _,
            path,
            allowed_origins,
            allow_missing_origin,
            protocols,
            max_messages,
            callback,
            callback_layout,
            websocket_class_id,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpServerWebSocketServeOnce",
                report,
            );
            let string_list = module
                .list_types
                .iter()
                .position(|element| *element == IrType::String)
                .map(|id| IrType::List(id as u32));
            for (operand, expected, label) in [
                (*stream_handle, IrType::Int(IntWidth::U64), "stream handle"),
                (*path, IrType::String, "path"),
                (
                    *allow_missing_origin,
                    IrType::Boolean,
                    "allow-missing-origin flag",
                ),
                (*max_messages, IrType::Int(IntWidth::U64), "max messages"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: HttpServerWebSocketServeOnce {label} has type {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            if let Some(expected) = string_list {
                for (operand, label) in [
                    (*allowed_origins, "allowed origins"),
                    (*protocols, "subprotocols"),
                ] {
                    if let Some(actual) = type_of(&operand)
                        && actual != expected
                    {
                        report(format!(
                            "{position}: HttpServerWebSocketServeOnce {label} has type {}, expected {}",
                            actual.as_str(),
                            expected.as_str()
                        ));
                    }
                }
            } else {
                report(format!(
                    "{position}: HttpServerWebSocketServeOnce requires List<String> in the module type table"
                ));
            }
            if *websocket_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpServerWebSocketServeOnce references missing HTTPWebSocket layout {websocket_class_id}"
                ));
            }
            let callback_ok =
                module
                    .closures
                    .get(*callback_layout as usize)
                    .is_some_and(|layout| {
                        layout.params == vec![IrType::Object(*websocket_class_id)]
                            && layout.returns == IrType::Void
                    });
            if !callback_ok {
                report(format!(
                    "{position}: HttpServerWebSocketServeOnce callback must accept HTTPWebSocket and return Void"
                ));
            }
            if let Some(actual) = type_of(callback)
                && actual != IrType::Callable(*callback_layout)
            {
                report(format!(
                    "{position}: HttpServerWebSocketServeOnce callback has type {}, expected callable layout {callback_layout}",
                    actual.as_str()
                ));
            }
        }

        InstKind::HttpServerWebSocketServeHttp3Once {
            endpoint,
            certificate_chain_der,
            private_key_der,
            path,
            allowed_origins,
            allow_missing_origin,
            protocols,
            max_messages,
            idle_timeout_ms,
            callback,
            callback_layout,
            websocket_class_id,
            enum_id,
        } => {
            expect(
                inst.ty,
                IrType::Enum(*enum_id),
                position,
                "HttpServerWebSocketServeHttp3Once",
                report,
            );
            for (operand, expected, label) in [
                (*endpoint, IrType::String, "endpoint"),
                (*path, IrType::String, "path"),
                (
                    *allow_missing_origin,
                    IrType::Boolean,
                    "allow-missing-origin flag",
                ),
                (*max_messages, IrType::Int(IntWidth::U64), "max messages"),
                (*idle_timeout_ms, IrType::Int(IntWidth::U64), "idle timeout"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && actual != expected
                {
                    report(format!(
                        "{position}: HttpServerWebSocketServeHttp3Once {label} has type {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
            for (operand, label) in [
                (*certificate_chain_der, "certificate chain"),
                (*private_key_der, "private key"),
            ] {
                if let Some(actual) = type_of(&operand)
                    && !matches!(actual, IrType::Array(_))
                {
                    report(format!(
                        "{position}: HttpServerWebSocketServeHttp3Once {label} must be an array, got {}",
                        actual.as_str()
                    ));
                }
            }
            let string_list = module
                .list_types
                .iter()
                .position(|element| *element == IrType::String)
                .map(|id| IrType::List(id as u32));
            if let Some(expected) = string_list {
                for (operand, label) in [
                    (*allowed_origins, "allowed origins"),
                    (*protocols, "subprotocols"),
                ] {
                    if let Some(actual) = type_of(&operand)
                        && actual != expected
                    {
                        report(format!(
                            "{position}: HttpServerWebSocketServeHttp3Once {label} has type {}, expected {}",
                            actual.as_str(),
                            expected.as_str()
                        ));
                    }
                }
            } else {
                report(format!(
                    "{position}: HttpServerWebSocketServeHttp3Once requires List<String> in the module type table"
                ));
            }
            if *websocket_class_id as usize >= module.objects.len() {
                report(format!(
                    "{position}: HttpServerWebSocketServeHttp3Once references missing HTTPWebSocket layout {websocket_class_id}"
                ));
            }
            let callback_ok =
                module
                    .closures
                    .get(*callback_layout as usize)
                    .is_some_and(|layout| {
                        layout.params == vec![IrType::Object(*websocket_class_id)]
                            && layout.returns == IrType::Void
                    });
            if !callback_ok {
                report(format!(
                    "{position}: HttpServerWebSocketServeHttp3Once callback must accept HTTPWebSocket and return Void"
                ));
            }
            if let Some(actual) = type_of(callback)
                && actual != IrType::Callable(*callback_layout)
            {
                report(format!(
                    "{position}: HttpServerWebSocketServeHttp3Once callback has type {}, expected callable layout {callback_layout}",
                    actual.as_str()
                ));
            }
        }

        InstKind::NullValue(base) => {
            expect(
                inst.ty,
                IrType::Nullable(*base),
                position,
                "NullValue",
                report,
            );
        }

        InstKind::Wrap { base, value } => {
            expect(inst.ty, IrType::Nullable(*base), position, "Wrap", report);
            if let Some(inner) = type_of(value)
                && inner != base.inner()
            {
                report(format!(
                    "{position}: Wrap receives {}, expected {}",
                    inner.as_str(),
                    base.inner().as_str()
                ));
            }
        }

        InstKind::IsNull(operand) => {
            expect(inst.ty, IrType::Boolean, position, "IsNull", report);
            if let Some(value) = type_of(operand)
                && !matches!(value, IrType::Nullable(_))
            {
                report(format!(
                    "{position}: IsNull receives {}, which is never absent",
                    value.as_str()
                ));
            }
        }

        InstKind::MakeClosure {
            id,
            captures,
            target,
        } => {
            expect(
                inst.ty,
                IrType::Closure(*id),
                position,
                "MakeClosure",
                report,
            );

            let Some(layout) = module.closures.get(*id as usize) else {
                report(format!(
                    "{position}: MakeClosure names layout {id}, which does not exist"
                ));
                return;
            };

            if module.function(target).is_none() {
                report(format!(
                    "{position}: MakeClosure targets `{target}`, which is not a module function"
                ));
                return;
            }

            if captures.len() != layout.captures.len() {
                report(format!(
                    "{position}: MakeClosure passes {} capture(s), the layout declares {}",
                    captures.len(),
                    layout.captures.len()
                ));
                return;
            }

            for (index, (operand, expected)) in captures.iter().zip(&layout.captures).enumerate() {
                if let Some(actual) = type_of(operand)
                    && actual != *expected
                {
                    report(format!(
                        "{position}: capture {index} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }

        InstKind::CallClosure { id, callee, args } => {
            let id = *id;
            // The operand must be the very closure the id names: otherwise the
            // captures extracted from it would not match what the body expects.
            if let Some(actual) = type_of(callee)
                && actual != IrType::Closure(id)
            {
                report(format!(
                    "{position}: CallClosure receives {}, expected {}",
                    actual.as_str(),
                    IrType::Closure(id).as_str()
                ));
                return;
            }

            let Some(layout) = module.closures.get(id as usize) else {
                report(format!(
                    "{position}: CallClosure names layout {id}, which does not exist"
                ));
                return;
            };

            if matches!(layout.params.first(), Some(IrType::BorrowedView { .. })) {
                report(format!(
                    "{position}: BorrowedView callbacks may only be invoked by MutexWithCall"
                ));
                return;
            }

            expect(inst.ty, layout.returns, position, "CallClosure", report);

            if args.len() != layout.params.len() {
                report(format!(
                    "{position}: CallClosure passes {} argument(s), the closure takes {}",
                    args.len(),
                    layout.params.len()
                ));
                return;
            }

            for (index, (operand, expected)) in args.iter().zip(&layout.params).enumerate() {
                if let Some(actual) = type_of(operand)
                    && actual != *expected
                {
                    report(format!(
                        "{position}: argument {index} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }

        InstKind::Unwrap(operand) => {
            // Unwrapping must produce exactly the type inside the operand:
            // that is what makes the representation change checkable rather
            // than implicit.
            if let Some(value) = type_of(operand) {
                match value {
                    IrType::Nullable(base) => {
                        expect(inst.ty, base.inner(), position, "Unwrap", report)
                    }
                    other => report(format!(
                        "{position}: Unwrap receives {}, which is not nullable",
                        other.as_str()
                    )),
                }
            }
        }

        InstKind::PointerFromSlot(slot) => {
            let Some(declared) = function.slot(*slot) else {
                report(format!(
                    "{position}: PointerFromSlot names slot {slot:?}, which does not exist"
                ));
                return;
            };
            if !matches!(inst.ty, IrType::Pointer(id) if module.pointer_types.get(id as usize) == Some(&declared.ty))
            {
                report(format!(
                    "{position}: PointerFromSlot declares {}, expected a Pointer to {}",
                    inst.ty.as_str(),
                    declared.ty.as_str()
                ));
            }
        }
        InstKind::PointerFromField { object, .. } => {
            if let Some(ty) = type_of(object) {
                match ty {
                    IrType::Pointer(id) => {
                        if !matches!(
                            module.pointer_types.get(id as usize),
                            Some(IrType::Object(_) | IrType::Value(_))
                        ) {
                            report(format!(
                                "{position}: PointerFromField reads {}, which is not a Pointer to an object or value",
                                ty.as_str()
                            ));
                        }
                    }
                    other => {
                        report(format!(
                            "{position}: PointerFromField reads {}, which is not a Pointer",
                            other.as_str()
                        ));
                    }
                }
            }
            if !matches!(inst.ty, IrType::Pointer(_)) {
                report(format!(
                    "{position}: PointerFromField declares {}, expected a Pointer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PointerRead(pointer) => {
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerRead reads through {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
        }
        InstKind::PointerWrite { pointer, .. } => {
            expect(inst.ty, IrType::Void, position, "PointerWrite", report);
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerWrite writes through {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
        }
        InstKind::PointerOffset { pointer, .. } | InstKind::PointerOffsetBytes { pointer, .. } => {
            if let Some(ty) = type_of(pointer)
                && ty != inst.ty
            {
                report(format!(
                    "{position}: pointer offset declares {}, expected {} (the operand's own type)",
                    inst.ty.as_str(),
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Pointer(_)) {
                report(format!(
                    "{position}: pointer offset declares {}, expected a Pointer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PointerCast(operand) => {
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerCast converts {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
            if !matches!(inst.ty, IrType::Pointer(_)) {
                report(format!(
                    "{position}: PointerCast declares {}, expected a Pointer",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PointerIsNull(operand) => {
            expect(inst.ty, IrType::Boolean, position, "PointerIsNull", report);
            if let Some(ty) = type_of(operand)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: PointerIsNull reads {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
        }

        InstKind::NativeSliceValidate {
            pointer, length, ..
        } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "NativeSliceValidate",
                report,
            );
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: NativeSliceValidate validates {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(length)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceValidate's length is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceValue { pointer, length } => {
            if !matches!(inst.ty, IrType::NativeSlice(_) | IrType::NativeSliceMut(_)) {
                report(format!(
                    "{position}: NativeSliceValue declares {}, expected a NativeSlice/NativeSliceMut",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(pointer)
                && !matches!(ty, IrType::Pointer(_))
            {
                report(format!(
                    "{position}: NativeSliceValue's pointer is {}, which is not a Pointer",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(length)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceValue's length is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceLength(receiver) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "NativeSliceLength",
                report,
            );
            if let Some(ty) = type_of(receiver)
                && !matches!(ty, IrType::NativeSlice(_) | IrType::NativeSliceMut(_))
            {
                report(format!(
                    "{position}: NativeSliceLength reads {}, which is not a NativeSlice/NativeSliceMut",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceLoad { receiver, index } => {
            if let Some(ty) = type_of(receiver)
                && !matches!(ty, IrType::NativeSlice(_) | IrType::NativeSliceMut(_))
            {
                report(format!(
                    "{position}: NativeSliceLoad reads {}, which is not a NativeSlice/NativeSliceMut",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceLoad's index is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }
        InstKind::NativeSliceStore {
            receiver,
            index,
            value: _,
        } => {
            expect(inst.ty, IrType::Void, position, "NativeSliceStore", report);
            if let Some(ty) = type_of(receiver)
                && !matches!(ty, IrType::NativeSliceMut(_))
            {
                report(format!(
                    "{position}: NativeSliceStore writes through {}, which is not a NativeSliceMut",
                    ty.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && ty != IrType::Int(IntWidth::U64)
            {
                report(format!(
                    "{position}: NativeSliceStore's index is {}, expected UInt64",
                    ty.as_str()
                ));
            }
        }

        InstKind::WeakFrom(value) => {
            if let Some(ty) = type_of(value)
                && !matches!(inst.ty, IrType::Weak(id) if module.weak_types.get(id as usize) == Some(&ty))
            {
                report(format!(
                    "{position}: WeakFrom wraps {}, declares {}, expected a Weak of the wrapped type",
                    ty.as_str(),
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::WeakUpgrade(weak) => match type_of(weak) {
            Some(IrType::Weak(id)) => {
                let expected = module
                    .weak_types
                    .get(id as usize)
                    .copied()
                    .and_then(Nullable::of)
                    .map(IrType::Nullable);
                if let Some(expected) = expected
                    && inst.ty != expected
                {
                    report(format!(
                        "{position}: WeakUpgrade declares {}, expected {}",
                        inst.ty.as_str(),
                        expected.as_str()
                    ));
                }
            }
            Some(other) => report(format!(
                "{position}: WeakUpgrade reads through {}, which is not a Weak",
                other.as_str()
            )),
            None => {}
        },
        InstKind::WeakIsAlive(weak) => {
            expect(inst.ty, IrType::Boolean, position, "WeakIsAlive", report);
            if let Some(ty) = type_of(weak)
                && !matches!(ty, IrType::Weak(_))
            {
                report(format!(
                    "{position}: WeakIsAlive reads {}, which is not a Weak",
                    ty.as_str()
                ));
            }
        }
        InstKind::Clone(value) => {
            if let Some(ty) = type_of(value)
                && inst.ty != ty
            {
                report(format!(
                    "{position}: Clone declares {}, expected {} (the receiver's own type)",
                    inst.ty.as_str(),
                    ty.as_str()
                ));
            }
        }

        InstKind::JournalBegin => {
            expect(
                inst.ty,
                IrType::JournalHandle,
                position,
                "JournalBegin",
                report,
            );
        }
        InstKind::JournalRecordSlot { journal, slot } => {
            expect(inst.ty, IrType::Void, position, "JournalRecordSlot", report);
            if let Some(ty) = type_of(journal)
                && ty != IrType::JournalHandle
            {
                report(format!(
                    "{position}: JournalRecordSlot's journal is {}, expected JournalHandle",
                    ty.as_str()
                ));
            }
            if function.slot(*slot).is_none() {
                report(format!(
                    "{position}: JournalRecordSlot names slot {slot:?}, which does not exist"
                ));
            }
        }
        InstKind::JournalRecordField {
            journal,
            object,
            index,
        } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "JournalRecordField",
                report,
            );
            if let Some(ty) = type_of(journal)
                && ty != IrType::JournalHandle
            {
                report(format!(
                    "{position}: JournalRecordField's journal is {}, expected JournalHandle",
                    ty.as_str()
                ));
            }
            if field_type(module, type_of(object), *index).is_none() {
                report(format!(
                    "{position}: JournalRecordField writes field {index} of something that is not an object with it"
                ));
            }
        }
        InstKind::JournalCommit(journal) | InstKind::JournalRollback(journal) => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "JournalCommit/Rollback",
                report,
            );
            if let Some(ty) = type_of(journal)
                && ty != IrType::JournalHandle
            {
                report(format!(
                    "{position}: journal commit/rollback operates on {}, expected JournalHandle",
                    ty.as_str()
                ));
            }
        }
        InstKind::MakeCallable { target, captures } => {
            let IrType::Callable(id) = inst.ty else {
                report(format!(
                    "{position}: MakeCallable returns {}, expected a Callable type",
                    inst.ty.as_str()
                ));
                return;
            };

            let Some(layout) = module.closures.get(id as usize) else {
                report(format!(
                    "{position}: MakeCallable names layout {id}, which does not exist"
                ));
                return;
            };

            if module.function(target).is_none() {
                report(format!(
                    "{position}: MakeCallable targets `{target}`, which is not a module function"
                ));
                return;
            }

            let target_function = module.function(target).expect("checked above");
            let expected_params: Vec<IrType> = layout
                .captures
                .iter()
                .chain(&layout.params)
                .copied()
                .collect();
            let actual_params: Option<Vec<IrType>> = target_function
                .params
                .iter()
                .map(|slot| target_function.slot(*slot).map(|slot| slot.ty))
                .collect();
            let is_mutex_callback = layout
                .params
                .first()
                .is_some_and(|ty| matches!(ty, IrType::BorrowedView { .. }));
            if is_mutex_callback
                && (actual_params.as_deref() != Some(expected_params.as_slice())
                    || target_function.return_type != layout.returns)
            {
                report(format!(
                    "{position}: MakeCallable target `{target}` does not match its closure layout signature"
                ));
            }

            if captures.len() != layout.captures.len() {
                report(format!(
                    "{position}: MakeCallable passes {} capture(s), the layout declares {}",
                    captures.len(),
                    layout.captures.len()
                ));
                return;
            }

            for (index, (operand, expected)) in captures.iter().zip(&layout.captures).enumerate() {
                if let Some(actual) = type_of(operand)
                    && actual != *expected
                {
                    report(format!(
                        "{position}: capture {index} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }
        InstKind::CallCallable { callable, args } => {
            let Some(actual) = type_of(callable) else {
                report(format!("{position}: CallCallable callable is undefined"));
                return;
            };
            let IrType::Callable(id) = actual else {
                report(format!(
                    "{position}: CallCallable receives {}, expected a Callable type",
                    actual.as_str()
                ));
                return;
            };

            let Some(layout) = module.closures.get(id as usize) else {
                report(format!(
                    "{position}: CallCallable names layout {id}, which does not exist"
                ));
                return;
            };

            if matches!(layout.params.first(), Some(IrType::BorrowedView { .. })) {
                report(format!(
                    "{position}: BorrowedView callbacks may only be invoked by MutexWithCall"
                ));
                return;
            }

            expect(inst.ty, layout.returns, position, "CallCallable", report);

            if args.len() != layout.params.len() {
                report(format!(
                    "{position}: CallCallable passes {} argument(s), the callable takes {}",
                    args.len(),
                    layout.params.len()
                ));
                return;
            }

            for (index, (operand, expected)) in args.iter().zip(&layout.params).enumerate() {
                if let Some(actual) = type_of(operand)
                    && actual != *expected
                {
                    report(format!(
                        "{position}: argument {index} is {}, expected {}",
                        actual.as_str(),
                        expected.as_str()
                    ));
                }
            }
        }
        InstKind::ThreadRun {
            callable_slot,
            result_slot,
            exception_slot,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U32),
                position,
                "ThreadRun",
                report,
            );
            let Some(IrType::Callable(layout_id)) =
                function.slot(*callable_slot).map(|slot| slot.ty)
            else {
                report(format!(
                    "{position}: ThreadRun callable slot is not a Callable"
                ));
                return;
            };
            let Some(layout) = module.closures.get(layout_id as usize) else {
                report(format!(
                    "{position}: ThreadRun names missing callable layout {layout_id}"
                ));
                return;
            };
            if !layout.params.is_empty() {
                report(format!(
                    "{position}: ThreadRun callable must take no parameters"
                ));
            }
            if !function.gc_roots.contains(callable_slot) {
                report(format!(
                    "{position}: ThreadRun callable slot is not a GC root"
                ));
            }
            match (layout.returns, result_slot) {
                (IrType::Void, None) => {}
                (IrType::Void, Some(_)) => report(format!(
                    "{position}: Void ThreadRun must not have a result slot"
                )),
                (_, None) => report(format!(
                    "{position}: value-returning ThreadRun requires a result slot"
                )),
                (expected, Some(slot)) => {
                    if function.slot(*slot).map(|slot| slot.ty) != Some(expected) {
                        report(format!(
                            "{position}: ThreadRun result slot type does not match {}",
                            expected.as_str()
                        ));
                    }
                    if expected.is_managed_reference(module) && !function.gc_roots.contains(slot) {
                        report(format!(
                            "{position}: managed ThreadRun result slot is not a GC root"
                        ));
                    }
                }
            }
            let throwable = module
                .objects
                .iter()
                .position(|object| object.name == "Throwable");
            if function.slot(*exception_slot).map(|slot| slot.ty)
                != throwable.map(|id| IrType::Object(id as u32))
            {
                report(format!(
                    "{position}: ThreadRun exception slot must be Throwable"
                ));
            }
            if !function.gc_roots.contains(exception_slot) {
                report(format!(
                    "{position}: ThreadRun exception slot is not a GC root"
                ));
            }
            if *callable_slot == *exception_slot
                || result_slot == &Some(*exception_slot)
                || result_slot == &Some(*callable_slot)
            {
                report(format!("{position}: ThreadRun slots must be distinct"));
            }
        }
        InstKind::ResourceTransfer { source } => {
            if let Some(ty) = type_of(source) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_)) {
                    report(format!(
                        "{position}: ResourceTransfer source is {}, expected an object or contract reference",
                        ty.as_str()
                    ));
                }
                if inst.ty != ty {
                    report(format!(
                        "{position}: ResourceTransfer declares type {} but its source is {}",
                        inst.ty.as_str(),
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: ResourceTransfer source is undefined"));
            }
        }
        InstKind::DependentFrom { base, field_ptr } => {
            if let Some(ty) = type_of(base) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_)) {
                    report(format!(
                        "{position}: DependentFrom base is {}, expected an object or contract reference",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: DependentFrom base is undefined"));
            }
            if let Some(ty) = type_of(field_ptr) {
                if !matches!(ty, IrType::Pointer(_)) {
                    report(format!(
                        "{position}: DependentFrom field_ptr is {}, expected Pointer",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: DependentFrom field_ptr is undefined"));
            }
            if !matches!(inst.ty, IrType::Dependent(_)) {
                report(format!(
                    "{position}: DependentFrom declares {}, expected Dependent",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PinObject { object } => {
            if !matches!(inst.ty, IrType::Pin(_)) {
                report(format!(
                    "{position}: PinObject declares {}, expected Pin",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(object) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_) | IrType::Pin(_)) {
                    report(format!(
                        "{position}: PinObject target is {}, expected an object/contract/Pin reference",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: PinObject target is undefined"));
            }
        }
        InstKind::UnpinObject { object } => {
            expect(inst.ty, IrType::Void, position, "Unpin", report);
            if let Some(ty) = type_of(object) {
                if !matches!(ty, IrType::Object(_) | IrType::Contract(_) | IrType::Pin(_)) {
                    report(format!(
                        "{position}: UnpinObject target is {}, expected an object/contract/Pin reference",
                        ty.as_str()
                    ));
                }
            } else {
                report(format!("{position}: UnpinObject target is undefined"));
            }
        }

        InstKind::ArrayLength(operand) => {
            if inst.ty != IrType::Int(IntWidth::U64) {
                report(format!(
                    "{position}: ArrayLength declares {}, expected UInt64",
                    inst.ty.as_str()
                ));
            }
            if !matches!(type_of(operand), Some(IrType::Array(_))) {
                report(format!("{position}: ArrayLength receiver is not an Array"));
            }
        }
        InstKind::ListLength(operand) => {
            if inst.ty != IrType::Int(IntWidth::U64) {
                report(format!(
                    "{position}: ListLength declares {}, expected UInt64",
                    inst.ty.as_str()
                ));
            }
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!("{position}: ListLength receiver is not a List"));
            }
        }
        InstKind::DequeLength(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "DequeLength",
                report,
            );
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!("{position}: DequeLength receiver is not a Deque"));
            }
        }
        InstKind::DequeIsEmpty(operand) => {
            expect(inst.ty, IrType::Boolean, position, "DequeIsEmpty", report);
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!("{position}: DequeIsEmpty receiver is not a Deque"));
            }
        }
        InstKind::DequeVersion(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "DequeVersion",
                report,
            );
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!("{position}: DequeVersion receiver is not a Deque"));
            }
        }
        InstKind::PriorityQueueLength(operand) | InstKind::PriorityQueueVersion(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "PriorityQueueLength/Version",
                report,
            );
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!(
                    "{position}: priority queue query receiver is not a PriorityQueue"
                ));
            }
        }
        InstKind::PriorityQueueIsEmpty(operand) => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "PriorityQueueIsEmpty",
                report,
            );
            if !matches!(type_of(operand), Some(IrType::List(_))) {
                report(format!(
                    "{position}: priority queue query receiver is not a PriorityQueue"
                ));
            }
        }

        InstKind::ArrayNew {
            element_id,
            capacity,
        } => {
            if inst.ty != IrType::Array(*element_id) {
                report(format!(
                    "{position}: ArrayNew declares {}, expected Array<{element_id}>",
                    inst.ty.as_str()
                ));
            }
            if let Some(ty) = type_of(capacity)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ArrayNew capacity is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListNew { element_id } => {
            if inst.ty != IrType::List(*element_id) {
                report(format!(
                    "{position}: ListNew declares {}, expected List<{element_id}>",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::DequeNew { element_id } => {
            if inst.ty != IrType::List(*element_id) {
                report(format!(
                    "{position}: DequeNew declares {}, expected Deque<{element_id}>",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::PriorityQueueNew { element_id, order } => {
            if inst.ty != IrType::List(*element_id) {
                report(format!(
                    "{position}: PriorityQueueNew declares {}, expected PriorityQueue<{element_id}>",
                    inst.ty.as_str()
                ));
            }
            let element = module
                .list_types
                .get(*element_id as usize)
                .copied()
                .unwrap_or(IrType::Void);
            let expected_order = match element {
                IrType::Boolean => Some(1),
                IrType::Char => Some(2),
                IrType::Int(IntWidth::I8) => Some(3),
                IrType::Int(IntWidth::I16) => Some(4),
                IrType::Int(IntWidth::I32) => Some(5),
                IrType::Int(IntWidth::I64) => Some(6),
                IrType::Int(IntWidth::I128) => Some(7),
                IrType::Int(IntWidth::U8) => Some(8),
                IrType::Int(IntWidth::U16) => Some(9),
                IrType::Int(IntWidth::U32) => Some(10),
                IrType::Int(IntWidth::U64) => Some(11),
                IrType::Int(IntWidth::U128) => Some(12),
                IrType::Float(FloatWidth::F16) => Some(13),
                IrType::Float(FloatWidth::F32) => Some(14),
                IrType::Float(FloatWidth::F64) => Some(15),
                IrType::Float(FloatWidth::F128) => Some(16),
                IrType::Decimal => Some(17),
                IrType::String => Some(18),
                _ => None,
            };
            if expected_order != Some(*order) {
                report(format!(
                    "{position}: PriorityQueue order code {order} is incompatible with element type {}",
                    element.as_str()
                ));
            }
        }
        InstKind::PriorityQueueEnqueue { receiver, value } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "PriorityQueueEnqueue",
                report,
            );
            let element = if let Some(IrType::List(id)) = type_of(receiver) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if element == IrType::Void {
                report(format!(
                    "{position}: enqueue receiver is not a PriorityQueue"
                ));
            }
            if let Some(actual) = type_of(value)
                && element != IrType::Void
                && actual != element
            {
                report(format!(
                    "{position}: PriorityQueueEnqueue value is {}, expected {}",
                    actual.as_str(),
                    element.as_str()
                ));
            }
        }
        InstKind::PriorityQueuePeek(receiver) | InstKind::PriorityQueueDequeue(receiver) => {
            let element = if let Some(IrType::List(id)) = type_of(receiver) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if element == IrType::Void || inst.ty != element {
                report(format!(
                    "{position}: priority queue read has an incompatible receiver or result type"
                ));
            }
        }
        InstKind::DequePushFront { receiver, value }
        | InstKind::DequePushBack { receiver, value } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "DequePush",
                report,
            );
            let element = if let Some(IrType::List(id)) = type_of(receiver) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if element == IrType::Void {
                report(format!("{position}: DequePush receiver is not a Deque"));
            }
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: DequePush value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
        }
        InstKind::DequeFront(receiver)
        | InstKind::DequeBack(receiver)
        | InstKind::DequePopFront(receiver)
        | InstKind::DequePopBack(receiver) => {
            let element = if let Some(IrType::List(id)) = type_of(receiver) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            expect(inst.ty, element, position, "DequeAccess", report);
            if element == IrType::Void {
                report(format!("{position}: DequeAccess receiver is not a Deque"));
            }
        }
        InstKind::DequeClear(receiver) => {
            expect(inst.ty, IrType::Void, position, "DequeClear", report);
            if !matches!(type_of(receiver), Some(IrType::List(_))) {
                report(format!("{position}: DequeClear receiver is not a Deque"));
            }
        }
        InstKind::MapNew { map_id } => {
            if inst.ty != IrType::Map(*map_id) {
                report(format!(
                    "{position}: MapNew declares {}, expected Map<{map_id}>",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::SetNew { set_id } => {
            if inst.ty != IrType::Set(*set_id) {
                report(format!(
                    "{position}: SetNew declares {}, expected Set<{set_id}>",
                    inst.ty.as_str()
                ));
            }
        }
        InstKind::ArrayListLoad { receiver, index } => {
            let element = match type_of(receiver).unwrap_or(IrType::Void) {
                IrType::Array(id) => module
                    .array_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                IrType::List(id) => module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                _ => IrType::Void,
            };
            if element != IrType::Void && inst.ty != element {
                report(format!(
                    "{position}: ArrayListLoad declares {}, expected {}",
                    inst.ty.as_str(),
                    element.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ArrayListLoad index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ArrayListStore {
            receiver,
            index,
            value,
        } => {
            expect(inst.ty, IrType::Void, position, "ArrayListStore", report);
            let element = match type_of(receiver).unwrap_or(IrType::Void) {
                IrType::Array(id) => module
                    .array_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                IrType::List(id) => module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void),
                _ => IrType::Void,
            };
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: ArrayListStore value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ArrayListStore index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListAdd { receiver, value } => {
            expect(inst.ty, IrType::Void, position, "ListAdd", report);
            let element = if let IrType::List(id) = type_of(receiver).unwrap_or(IrType::Void) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: ListAdd value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
        }
        InstKind::ListInsert {
            receiver,
            index,
            value,
        } => {
            expect(inst.ty, IrType::Void, position, "ListInsert", report);
            let element = if let IrType::List(id) = type_of(receiver).unwrap_or(IrType::Void) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && element != IrType::Void
                && ty != element
            {
                report(format!(
                    "{position}: ListInsert value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ListInsert index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListRemove { receiver, index } => {
            expect(inst.ty, IrType::Void, position, "ListRemove", report);
            if !matches!(type_of(receiver).unwrap_or(IrType::Void), IrType::List(_)) {
                report(format!("{position}: ListRemove receiver is not a List"));
            }
            if let Some(ty) = type_of(index)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ListRemove index is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::ListRemoveValue { receiver, value } => {
            expect(
                inst.ty,
                IrType::Boolean,
                position,
                "ListRemoveValue",
                report,
            );
            let element = if let Some(IrType::List(id)) = type_of(receiver) {
                module
                    .list_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if element == IrType::Void {
                report(format!(
                    "{position}: ListRemoveValue receiver is not a List"
                ));
            }
            if let Some(ty) = type_of(value)
                && ty != element
            {
                report(format!(
                    "{position}: ListRemoveValue value is {}, expected {}",
                    ty.as_str(),
                    element.as_str()
                ));
            }
        }
        InstKind::ArrayClone { receiver } => {
            let expected = if let Some(IrType::Array(id)) = type_of(receiver) {
                IrType::Array(id)
            } else {
                report(format!("{position}: ArrayClone receiver is not an Array"));
                inst.ty
            };
            if inst.ty != expected {
                report(format!(
                    "{position}: ArrayClone declares {}, expected {}",
                    inst.ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::ListClone { receiver } => {
            let expected = if let Some(IrType::List(id)) = type_of(receiver) {
                IrType::List(id)
            } else {
                report(format!("{position}: ListClone receiver is not a List"));
                inst.ty
            };
            if inst.ty != expected {
                report(format!(
                    "{position}: ListClone declares {}, expected {}",
                    inst.ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::MapLength(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "MapLength",
                report,
            );
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Map(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: MapLength receiver is not a Map"));
            }
        }
        InstKind::MapVersion(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "MapVersion",
                report,
            );
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Map(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: MapVersion receiver is not a Map"));
            }
        }
        InstKind::MapKeys(operand)
        | InstKind::MapSortedKeys(operand)
        | InstKind::MapValues(operand) => {
            let receiver_ty = type_of(operand);
            let expected_element = match receiver_ty {
                Some(IrType::Map(id)) => module.map_types.get(id as usize).map(|(key, value)| {
                    if matches!(inst.kind, InstKind::MapKeys(_) | InstKind::MapSortedKeys(_)) {
                        *key
                    } else {
                        *value
                    }
                }),
                Some(_) => {
                    report(format!("{position}: map snapshot receiver is not a Map"));
                    None
                }
                None => None,
            };
            let actual_element = match inst.ty {
                IrType::Array(id) => module.array_types.get(id as usize).copied(),
                _ => {
                    report(format!("{position}: map snapshot result is not an Array"));
                    None
                }
            };
            if expected_element.is_some() && actual_element != expected_element {
                report(format!(
                    "{position}: map snapshot element is {}, expected {}",
                    actual_element.map_or_else(|| "<invalid>".into(), |ty| ty.as_str()),
                    expected_element.expect("checked above").as_str()
                ));
            }
        }
        InstKind::MapEntries(operand) => {
            let pair = match type_of(operand) {
                Some(IrType::Map(id)) => module.map_types.get(id as usize).copied(),
                Some(_) => {
                    report(format!("{position}: MapEntries receiver is not a Map"));
                    None
                }
                None => None,
            };
            let actual = match inst.ty {
                IrType::Array(id) => module.array_types.get(id as usize).copied(),
                _ => {
                    report(format!("{position}: MapEntries result is not an Array"));
                    None
                }
            };
            let correct_tuple = pair.is_some_and(|(key, value)| {
                matches!(actual, Some(IrType::Value(tuple_id))
                if module.values.get(tuple_id as usize).is_some_and(|layout| {
                    layout.fields.len() == 2
                        && layout.fields[0].ty == key
                        && layout.fields[1].ty == value
                }))
            });
            if pair.is_some() && !correct_tuple {
                report(format!(
                    "{position}: MapEntries element is not the matching (K, V) tuple"
                ));
            }
        }
        InstKind::MapIsEmpty(operand) => {
            expect(inst.ty, IrType::Boolean, position, "MapIsEmpty", report);
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Map(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: MapIsEmpty receiver is not a Map"));
            }
        }
        InstKind::MapClear(operand) => {
            expect(inst.ty, IrType::Void, position, "MapClear", report);
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Map(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: MapClear receiver is not a Map"));
            }
        }
        InstKind::MapSet {
            receiver,
            key,
            value,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "MapSet",
                report,
            );
            let (expected_key, expected_value) = if let Some(IrType::Map(id)) = type_of(receiver) {
                module
                    .map_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or((IrType::Void, IrType::Void))
            } else {
                (IrType::Void, IrType::Void)
            };
            if let Some(ty) = type_of(key)
                && ty != expected_key
            {
                report(format!(
                    "{position}: MapSet key is {}, expected {}",
                    ty.as_str(),
                    expected_key.as_str()
                ));
            }
            if let Some(ty) = type_of(value)
                && ty != expected_value
            {
                report(format!(
                    "{position}: MapSet value is {}, expected {}",
                    ty.as_str(),
                    expected_value.as_str()
                ));
            }
        }
        InstKind::MapContainsKey { receiver, key } => {
            expect(inst.ty, IrType::Boolean, position, "MapContainsKey", report);
            let expected_key = if let Some(IrType::Map(id)) = type_of(receiver) {
                module
                    .map_types
                    .get(id as usize)
                    .map(|(k, _)| *k)
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(key)
                && ty != expected_key
            {
                report(format!(
                    "{position}: MapContainsKey key is {}, expected {}",
                    ty.as_str(),
                    expected_key.as_str()
                ));
            }
        }
        InstKind::SetLength(operand) => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "SetLength",
                report,
            );
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Set(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: SetLength receiver is not a Set"));
            }
        }
        InstKind::SetValues(operand) | InstKind::SetSortedElements(operand) => {
            let element = match type_of(operand) {
                Some(IrType::Set(id)) => module.set_types.get(id as usize).copied(),
                Some(_) => {
                    report(format!("{position}: SetValues receiver is not a Set"));
                    None
                }
                None => None,
            };
            let actual = match inst.ty {
                IrType::Array(id) => module.array_types.get(id as usize).copied(),
                _ => {
                    report(format!("{position}: SetValues result is not an Array"));
                    None
                }
            };
            if element.is_some() && actual != element {
                report(format!(
                    "{position}: SetValues element is {}, expected {}",
                    actual.map_or_else(|| "<invalid>".into(), |ty| ty.as_str()),
                    element.expect("checked above").as_str()
                ));
            }
        }
        InstKind::SetIsEmpty(operand) => {
            expect(inst.ty, IrType::Boolean, position, "SetIsEmpty", report);
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Set(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: SetIsEmpty receiver is not a Set"));
            }
        }
        InstKind::SetClear(operand) => {
            expect(inst.ty, IrType::Void, position, "SetClear", report);
            if type_of(operand)
                .map(|ty| !matches!(ty, IrType::Set(_)))
                .unwrap_or(false)
            {
                report(format!("{position}: SetClear receiver is not a Set"));
            }
        }
        InstKind::SetOperation {
            left,
            right,
            operation,
        } => {
            let expected = match (type_of(left), type_of(right)) {
                (Some(IrType::Set(left_id)), Some(IrType::Set(right_id))) => {
                    let left_element = module.set_types.get(left_id as usize).copied();
                    let right_element = module.set_types.get(right_id as usize).copied();
                    if left_element != right_element {
                        report(format!("{position}: SetOperation element types differ"));
                    }
                    if matches!(
                        operation,
                        crate::SetOperation::Union
                            | crate::SetOperation::Intersection
                            | crate::SetOperation::Difference
                            | crate::SetOperation::SymmetricDifference
                    ) {
                        IrType::Set(left_id)
                    } else {
                        IrType::Boolean
                    }
                }
                _ => {
                    report(format!(
                        "{position}: SetOperation operands are not matching Sets"
                    ));
                    IrType::Boolean
                }
            };
            expect(inst.ty, expected, position, "SetOperation", report);
        }
        InstKind::SetAdd { receiver, value } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "SetAdd",
                report,
            );
            let expected = if let Some(IrType::Set(id)) = type_of(receiver) {
                module
                    .set_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && ty != expected
            {
                report(format!(
                    "{position}: SetAdd value is {}, expected {}",
                    ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::SetContains { receiver, value } => {
            expect(inst.ty, IrType::Boolean, position, "SetContains", report);
            let expected = if let Some(IrType::Set(id)) = type_of(receiver) {
                module
                    .set_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && ty != expected
            {
                report(format!(
                    "{position}: SetContains value is {}, expected {}",
                    ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::MapGet { receiver, key } => {
            let (expected_key, expected_value) = if let Some(IrType::Map(id)) = type_of(receiver) {
                module
                    .map_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or((IrType::Void, IrType::Void))
            } else {
                (IrType::Void, IrType::Void)
            };
            if inst.ty != expected_value {
                report(format!(
                    "{position}: MapGet declares {}, expected {}",
                    inst.ty.as_str(),
                    expected_value.as_str()
                ));
            }
            if let Some(ty) = type_of(key)
                && ty != expected_key
            {
                report(format!(
                    "{position}: MapGet key is {}, expected {}",
                    ty.as_str(),
                    expected_key.as_str()
                ));
            }
        }
        InstKind::MapRemove { receiver, key } => {
            expect(inst.ty, IrType::Boolean, position, "MapRemove", report);
            let expected_key = if let Some(IrType::Map(id)) = type_of(receiver) {
                module
                    .map_types
                    .get(id as usize)
                    .map(|(k, _)| *k)
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(key)
                && ty != expected_key
            {
                report(format!(
                    "{position}: MapRemove key is {}, expected {}",
                    ty.as_str(),
                    expected_key.as_str()
                ));
            }
        }
        InstKind::SetRemove { receiver, value } => {
            expect(inst.ty, IrType::Boolean, position, "SetRemove", report);
            let expected = if let Some(IrType::Set(id)) = type_of(receiver) {
                module
                    .set_types
                    .get(id as usize)
                    .copied()
                    .unwrap_or(IrType::Void)
            } else {
                IrType::Void
            };
            if let Some(ty) = type_of(value)
                && ty != expected
            {
                report(format!(
                    "{position}: SetRemove value is {}, expected {}",
                    ty.as_str(),
                    expected.as_str()
                ));
            }
        }
        InstKind::ArraySlice {
            receiver,
            start,
            end,
            step,
        } => {
            let expected = if let Some(IrType::Array(id)) = type_of(receiver) {
                IrType::Array(id)
            } else {
                report(format!("{position}: ArraySlice receiver is not an Array"));
                inst.ty
            };
            if inst.ty != expected {
                report(format!(
                    "{position}: ArraySlice declares {}, expected {}",
                    inst.ty.as_str(),
                    expected.as_str()
                ));
            }
            for (name, op) in [("start", start), ("end", end), ("step", step)] {
                if let Some(ty) = type_of(op)
                    && !matches!(ty, IrType::Int(_))
                {
                    report(format!(
                        "{position}: ArraySlice {name} is {}, expected an integer",
                        ty.as_str()
                    ));
                }
            }
        }

        InstKind::ScopeEnter => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I64),
                position,
                "ScopeEnter",
                report,
            );
        }
        InstKind::ScopeEnterCollectAll => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I64),
                position,
                "ScopeEnterCollectAll",
                report,
            );
        }
        InstKind::ScopeExit { scope } => {
            expect(inst.ty, IrType::Void, position, "ScopeExit", report);
            if let Some(ty) = type_of(scope)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: ScopeExit's scope is {}, expected the Int64 id from ScopeEnter",
                    ty.as_str()
                ));
            }
        }
        InstKind::ScopeExitCollectAll { scope } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "ScopeExitCollectAll",
                report,
            );
            if let Some(ty) = type_of(scope)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: ScopeExitCollectAll's scope is {}, expected the Int64 id from ScopeEnterCollectAll",
                    ty.as_str()
                ));
            }
        }
        InstKind::ScopeCancel { scope } => {
            expect(inst.ty, IrType::Void, position, "ScopeCancel", report);
            if let Some(ty) = type_of(scope)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: ScopeCancel's scope is {}, expected Int64",
                    ty.as_str()
                ));
            }
        }
        InstKind::ScopeOutcomeWaitTake {
            scope,
            ordinal_slot,
            value_slot,
            throwable_slot,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U32),
                position,
                "ScopeOutcomeWaitTake",
                report,
            );
            if let Some(ty) = type_of(scope)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: ScopeOutcomeWaitTake scope is {}, expected Int64",
                    ty.as_str()
                ));
            }
            if ordinal_slot == value_slot
                || ordinal_slot == throwable_slot
                || value_slot == throwable_slot
            {
                report(format!(
                    "{position}: scope outcome output slots must be distinct"
                ));
            }
            if let Some(slot) = function.slots.get(ordinal_slot.0 as usize) {
                if slot.ty != IrType::Int(IntWidth::U64) {
                    report(format!(
                        "{position}: scope outcome ordinal slot is {}, expected UInt64",
                        slot.ty.as_str()
                    ));
                }
            } else {
                report(format!(
                    "{position}: scope outcome ordinal slot {ordinal_slot:?} does not exist"
                ));
            }
            for (name, slot_id) in [("value", value_slot), ("throwable", throwable_slot)] {
                let Some(slot) = function.slots.get(slot_id.0 as usize) else {
                    report(format!(
                        "{position}: scope outcome {name} slot {slot_id:?} does not exist"
                    ));
                    continue;
                };
                let pointer_reference = matches!(
                    slot.ty,
                    IrType::Object(_)
                        | IrType::Contract(_)
                        | IrType::String
                        | IrType::Regex
                        | IrType::Char
                        | IrType::Array(_)
                        | IrType::List(_)
                        | IrType::Channel(_)
                        | IrType::Weak(_)
                        | IrType::Dependent(_)
                        | IrType::Pin(_)
                        | IrType::Map(_)
                        | IrType::Set(_)
                ) && slot.ty.is_managed_reference(module);
                if !pointer_reference {
                    report(format!(
                        "{position}: scope outcome {name} slot has non-pointer managed-reference type {}",
                        slot.ty.as_str()
                    ));
                }
                if !function.gc_roots.contains(slot_id) {
                    report(format!(
                        "{position}: scope outcome {name} slot {slot_id:?} is missing from the function's GC roots"
                    ));
                }
            }
        }
        InstKind::BranchStart {
            target,
            body,
            scope,
            kind,
            delay,
            status_slot,
        } => {
            if inst.ty != IrType::Job {
                report(format!(
                    "{position}: BranchStart declares {}, expected Job",
                    inst.ty.as_str()
                ));
            }
            if module.function(target).is_none() {
                report(format!(
                    "{position}: BranchStart targets `{target}`, which is not a module function"
                ));
            }
            if !matches!(type_of(body), Some(IrType::Callable(_))) {
                report(format!("{position}: BranchStart body is not a Callable"));
            }
            if function
                .slots
                .get(status_slot.0 as usize)
                .map(|slot| slot.ty)
                != Some(IrType::Int(IntWidth::U32))
            {
                report(format!("{position}: BranchStart status slot must be U32"));
            }
            if let Some(ty) = type_of(scope)
                && ty != IrType::Int(IntWidth::I64)
            {
                report(format!(
                    "{position}: BranchStart's scope is {}, expected the Int64 id from ScopeEnter",
                    ty.as_str()
                ));
            }
            match (kind.is_ambient(), delay) {
                (true, None) => report(format!(
                    "{position}: an ambient BranchStart ({kind:?}) needs a delay operand"
                )),
                (false, Some(_)) => {
                    report(format!("{position}: a Spawn BranchStart carries no delay"))
                }
                (true, Some(operand)) => {
                    if let Some(ty) = type_of(operand)
                        && !matches!(ty, IrType::Int(_))
                    {
                        report(format!(
                            "{position}: BranchStart delay is {}, expected an integer",
                            ty.as_str()
                        ));
                    }
                }
                (false, None) => {}
            }
        }
        InstKind::BackgroundStart {
            target,
            body,
            status_slot,
        } => {
            expect(inst.ty, IrType::Void, position, "BackgroundStart", report);
            if module.function(target).is_none() {
                report(format!(
                    "{position}: BackgroundStart targets `{target}`, which is not a module function"
                ));
            }
            if !matches!(type_of(body), Some(IrType::Callable(_))) {
                report(format!(
                    "{position}: BackgroundStart body is not a Callable"
                ));
            }
            if function
                .slots
                .get(status_slot.0 as usize)
                .map(|slot| slot.ty)
                != Some(IrType::Int(IntWidth::U32))
            {
                report(format!(
                    "{position}: BackgroundStart status slot must be U32"
                ));
            }
            if module
                .function(target)
                .is_some_and(|target| target.return_type != IrType::Void)
            {
                report(format!(
                    "{position}: BackgroundStart target must return Void"
                ));
            }
        }
        InstKind::ServiceStart {
            target,
            body,
            status_slot,
        } => {
            expect(inst.ty, IrType::Void, position, "ServiceStart", report);
            if module.function(target).is_none() {
                report(format!(
                    "{position}: ServiceStart target `{target}` is not a module function"
                ));
            }
            let layout = match type_of(body) {
                Some(IrType::Callable(id)) => module.closures.get(id as usize),
                _ => None,
            };
            let Some(layout) = layout else {
                report(format!(
                    "{position}: ServiceStart body is not a valid Callable"
                ));
                return;
            };
            if function.slot(*status_slot).map(|slot| slot.ty) != Some(IrType::Int(IntWidth::U32)) {
                report(format!("{position}: ServiceStart status slot must be U32"));
            }
            if layout.params != [IrType::Int(IntWidth::U64)] || layout.returns != IrType::Void {
                report(format!(
                    "{position}: ServiceStart callback must have signature (ServiceContext): Void"
                ));
            }
            if let Some(target_function) = module.function(target) {
                let mut expected = layout.captures.clone();
                expected.extend(layout.params.iter().copied());
                let actual: Vec<_> = target_function
                    .params
                    .iter()
                    .filter_map(|slot| target_function.slot(*slot).map(|slot| slot.ty))
                    .collect();
                if target_function.return_type != IrType::Void || actual != expected {
                    report(format!(
                        "{position}: ServiceStart target signature does not match its callback layout"
                    ));
                }
            }
        }
        InstKind::ServiceReady {
            context,
            status_slot,
        } => {
            expect(inst.ty, IrType::Void, position, "ServiceReady", report);
            if type_of(context) != Some(IrType::Int(IntWidth::U64)) {
                report(format!("{position}: ServiceReady context must be U64"));
            }
            if function.slot(*status_slot).map(|slot| slot.ty) != Some(IrType::Int(IntWidth::U32)) {
                report(format!("{position}: ServiceReady status slot must be U32"));
            }
        }
        InstKind::ParallelForStart {
            target,
            body,
            count,
            budget_kind,
            budget_a,
            budget_b,
        } => {
            expect(inst.ty, IrType::Void, position, "ParallelForStart", report);
            if module.function(target).is_none() {
                report(format!(
                    "{position}: ParallelForStart targets `{target}`, which is not a module function"
                ));
            }
            if !matches!(type_of(body), Some(IrType::Callable(_))) {
                report(format!(
                    "{position}: ParallelForStart body is not a Callable"
                ));
            }
            if let Some(ty) = type_of(count)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: ParallelForStart count is {}, expected an integer",
                    ty.as_str()
                ));
            }
            if *budget_kind > 2 {
                report(format!(
                    "{position}: ParallelForStart budget_kind is {budget_kind}, expected 0, 1, or 2"
                ));
            }
            for (name, operand) in [("budget_a", budget_a), ("budget_b", budget_b)] {
                if let Some(ty) = type_of(operand)
                    && !matches!(ty, IrType::Int(_))
                {
                    report(format!(
                        "{position}: ParallelForStart {name} is {}, expected an integer",
                        ty.as_str()
                    ));
                }
            }
        }
        InstKind::JobWait { job, result } => {
            if inst.ty != *result {
                report(format!(
                    "{position}: JobWait declares {} but its result type is {}",
                    inst.ty.as_str(),
                    result.as_str()
                ));
            }
            if !matches!(type_of(job), Some(IrType::Job)) {
                report(format!("{position}: JobWait operand is not a Job"));
            }
        }
        InstKind::JobWaitAny { jobs } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "JobWaitAny",
                report,
            );
            if !(2..=8).contains(&jobs.len()) {
                report(format!(
                    "{position}: JobWaitAny has {} operands, expected 2..=8",
                    jobs.len()
                ));
            }
            for (index, job) in jobs.iter().enumerate() {
                if !matches!(type_of(job), Some(IrType::Job)) {
                    report(format!(
                        "{position}: JobWaitAny operand {} is not a Job",
                        index + 1
                    ));
                }
            }
        }
        InstKind::JobWaitAnyWithTimer { jobs } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U64),
                position,
                "JobWaitAnyWithTimer",
                report,
            );
            if !(2..=9).contains(&jobs.len()) {
                report(format!(
                    "{position}: JobWaitAnyWithTimer has {} operands, expected 2..=9",
                    jobs.len()
                ));
            }
            for (index, job) in jobs.iter().enumerate() {
                if !matches!(type_of(job), Some(IrType::Job)) {
                    report(format!(
                        "{position}: JobWaitAnyWithTimer operand {} is not a Job",
                        index + 1
                    ));
                }
            }
        }
        InstKind::JobOutcomeWaitTake {
            job,
            value_slot,
            throwable_slot,
        }
        | InstKind::JobOutcomeTake {
            job,
            value_slot,
            throwable_slot,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U32),
                position,
                "JobOutcomeTake",
                report,
            );
            if !matches!(type_of(job), Some(IrType::Job)) {
                report(format!("{position}: outcome take operand is not a Job"));
            }
            if value_slot == throwable_slot {
                report(format!(
                    "{position}: outcome take output slots must be distinct"
                ));
            }
            for (name, slot_id) in [("value", value_slot), ("throwable", throwable_slot)] {
                let Some(slot) = function.slots.get(slot_id.0 as usize) else {
                    report(format!(
                        "{position}: outcome take {name} slot {slot_id:?} does not exist"
                    ));
                    continue;
                };
                // The runtime ABI writes through `void **`. Requiring a
                // pointer-shaped managed reference avoids treating an inline
                // aggregate that merely contains references as an out-slot.
                let pointer_reference = matches!(
                    slot.ty,
                    IrType::Object(_)
                        | IrType::Contract(_)
                        | IrType::String
                        | IrType::Regex
                        | IrType::Char
                        | IrType::Array(_)
                        | IrType::List(_)
                        | IrType::Channel(_)
                        | IrType::Weak(_)
                        | IrType::Dependent(_)
                        | IrType::Pin(_)
                        | IrType::Map(_)
                        | IrType::Set(_)
                ) && slot.ty.is_managed_reference(module);
                if !pointer_reference {
                    report(format!(
                        "{position}: outcome take {name} slot has non-pointer managed-reference type {}",
                        slot.ty.as_str()
                    ));
                }
                if !function.gc_roots.contains(slot_id) {
                    report(format!(
                        "{position}: outcome take {name} slot {slot_id:?} is missing from the function's GC roots"
                    ));
                }
            }
        }
        InstKind::JobResultLoad {
            box_pointer,
            result,
        } => {
            expect(inst.ty, *result, position, "JobResultLoad", report);
            if matches!(result, IrType::Void | IrType::Never | IrType::Job) {
                report(format!(
                    "{position}: JobResultLoad result type {} cannot be boxed as a task payload",
                    result.as_str()
                ));
            }
            if !matches!(
                type_of(box_pointer),
                Some(
                    IrType::Object(_)
                        | IrType::Contract(_)
                        | IrType::String
                        | IrType::Array(_)
                        | IrType::List(_)
                        | IrType::Channel(_)
                        | IrType::Map(_)
                        | IrType::Set(_)
                        | IrType::Weak(_)
                        | IrType::Dependent(_)
                        | IrType::Pin(_)
                        | IrType::Regex
                        | IrType::Char
                )
            ) {
                report(format!(
                    "{position}: JobResultLoad box pointer is not a pointer-shaped managed reference"
                ));
            }
        }
        InstKind::JobDone { job } => {
            expect(inst.ty, IrType::Boolean, position, "JobDone", report);
            if !matches!(type_of(job), Some(IrType::Job)) {
                report(format!("{position}: JobDone operand is not a Job"));
            }
        }
        InstKind::JobCancel { job } => {
            expect(inst.ty, IrType::Void, position, "JobCancel", report);
            if !matches!(type_of(job), Some(IrType::Job)) {
                report(format!("{position}: JobCancel operand is not a Job"));
            }
        }
        InstKind::JobCancelWithCleanupFailure { job } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "JobCancelWithCleanupFailure",
                report,
            );
            if !matches!(type_of(job), Some(IrType::Job)) {
                report(format!(
                    "{position}: cleanup-failure cancellation operand is not a Job"
                ));
            }
        }
        InstKind::TimerSleep { nanos } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U32),
                position,
                "TimerSleep",
                report,
            );
            if let Some(ty) = type_of(nanos)
                && !matches!(ty, IrType::Int(_))
            {
                report(format!(
                    "{position}: TimerSleep delay is {}, expected an integer",
                    ty.as_str()
                ));
            }
        }
        InstKind::AppSetShutdownGrace { nanos } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::U32),
                position,
                "AppSetShutdownGrace",
                report,
            );
            if let Some(ty) = type_of(nanos)
                && !matches!(ty, IrType::Int(IntWidth::I64))
            {
                report(format!(
                    "{position}: AppSetShutdownGrace duration is {}, expected i64 nanoseconds",
                    ty.as_str()
                ));
            }
        }
        InstKind::AppSetBackgroundError {
            callable, layout, ..
        } => {
            expect(
                inst.ty,
                IrType::Void,
                position,
                "AppSetBackgroundError",
                report,
            );
            if !matches!(type_of(callable), Some(IrType::Callable(id)) if id == *layout) {
                report(format!(
                    "{position}: AppSetBackgroundError callable/layout mismatch"
                ));
            }
        }
        InstKind::AtomicNew {
            kind,
            value,
            object,
        } => {
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "AtomicNew",
                report,
            );
            if type_of(value) != Some((*kind).ir_type()) {
                report(format!(
                    "{position}: AtomicNew value does not match {:?}",
                    kind
                ));
            }
            match function.slot(*object) {
                Some(slot) if slot.ty == IrType::Atomic(*kind) => {
                    if !function.gc_roots.contains(object) {
                        report(format!(
                            "{position}: AtomicNew destination is not a GC root"
                        ));
                    }
                }
                Some(_) => report(format!(
                    "{position}: AtomicNew destination slot has the wrong Atomic kind"
                )),
                None => report(format!(
                    "{position}: AtomicNew destination slot does not exist"
                )),
            }
        }
        InstKind::AtomicLoad { object, order, out } => {
            verify_atomic_receiver(function, type_of(object), position, report);
            verify_atomic_order(type_of, *order, position, report);
            verify_atomic_output(function, *out, type_of(object), position, report);
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "AtomicLoad",
                report,
            );
        }
        InstKind::AtomicStore {
            object,
            value,
            order,
        } => {
            verify_atomic_receiver(function, type_of(object), position, report);
            verify_atomic_value(type_of(object), type_of(value), position, report);
            verify_atomic_order(type_of, *order, position, report);
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "AtomicStore",
                report,
            );
        }
        InstKind::AtomicExchange {
            object,
            value,
            order,
            out,
        }
        | InstKind::AtomicFetchAdd {
            object,
            value,
            order,
            out,
        }
        | InstKind::AtomicFetchSub {
            object,
            value,
            order,
            out,
        } => {
            verify_atomic_receiver(function, type_of(object), position, report);
            verify_atomic_value(type_of(object), type_of(value), position, report);
            verify_atomic_order(type_of, *order, position, report);
            verify_atomic_output(function, *out, type_of(object), position, report);
            if matches!(
                inst.kind,
                InstKind::AtomicFetchAdd { .. } | InstKind::AtomicFetchSub { .. }
            ) && !matches!(
                type_of(object),
                Some(IrType::Atomic(
                    crate::AtomicKind::I8
                        | crate::AtomicKind::U8
                        | crate::AtomicKind::I16
                        | crate::AtomicKind::U16
                        | crate::AtomicKind::I32
                        | crate::AtomicKind::U32
                        | crate::AtomicKind::I64
                        | crate::AtomicKind::U64
                ))
            ) {
                report(format!(
                    "{position}: fetch add/sub requires an integer Atomic"
                ));
            }
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "Atomic operation",
                report,
            );
        }
        InstKind::AtomicCompareExchange {
            object,
            current,
            new_value,
            success_order,
            failure_order,
            observed,
            exchanged,
        } => {
            verify_atomic_receiver(function, type_of(object), position, report);
            verify_atomic_value(type_of(object), type_of(current), position, report);
            verify_atomic_value(type_of(object), type_of(new_value), position, report);
            verify_atomic_order(type_of, Some(*success_order), position, report);
            verify_atomic_order(type_of, Some(*failure_order), position, report);
            verify_atomic_output(function, *observed, type_of(object), position, report);
            if function.slot(*exchanged).map(|slot| slot.ty) != Some(IrType::Boolean) {
                report(format!(
                    "{position}: compare-exchange flag slot must be Boolean"
                ));
            }
            expect(
                inst.ty,
                IrType::Int(IntWidth::I32),
                position,
                "AtomicCompareExchange",
                report,
            );
        }
        InstKind::ChannelNew {
            capacity,
            limit,
            element: _,
        } => {
            if !matches!(type_of(capacity), Some(IrType::Int(_)))
                || !matches!(type_of(limit), Some(IrType::Int(_)))
            {
                report(format!(
                    "{position}: ChannelNew capacity and limit must be integers"
                ));
            }
            if !matches!(inst.ty, IrType::Channel(_)) {
                report(format!("{position}: ChannelNew must produce a Channel"));
            }
        }
        InstKind::ChannelSend { channel, value: _ }
        | InstKind::ChannelTrySend { channel, value: _ }
        | InstKind::ChannelClose { channel } => {
            if !matches!(type_of(channel), Some(IrType::Channel(_))) {
                report(format!(
                    "{position}: channel operation receiver is not a Channel"
                ));
            }
            if matches!(inst.kind, InstKind::ChannelSend { .. })
                && !matches!(inst.ty, IrType::Int(_))
            {
                report(format!("{position}: ChannelSend must produce its status"));
            }
        }
        InstKind::ChannelRecv { channel, element } => {
            if !matches!(type_of(channel), Some(IrType::Channel(_))) {
                report(format!(
                    "{position}: channel operation receiver is not a Channel"
                ));
            }
            let expected = Nullable::of(*element).map(IrType::Nullable);
            if Some(inst.ty) != expected {
                report(format!(
                    "{position}: ChannelRecv must produce the nullable channel element"
                ));
            }
        }
        InstKind::ChannelTryRecv { channel, .. } => {
            if !matches!(type_of(channel), Some(IrType::Channel(_))) {
                report(format!(
                    "{position}: channel operation receiver is not a Channel"
                ));
            }
            if !matches!(inst.ty, IrType::Int(_) | IrType::Enum(_)) {
                report(format!(
                    "{position}: ChannelTryRecv status must be an integer"
                ));
            }
        }
        InstKind::ChannelLength(channel)
        | InstKind::ChannelIsClosed(channel)
        | InstKind::ChannelCapacity(channel) => {
            if !matches!(type_of(channel), Some(IrType::Channel(_))) {
                report(format!(
                    "{position}: channel property receiver is not a Channel"
                ));
            }
        }
        InstKind::ChannelDrain { channel, .. } => {
            if !matches!(type_of(channel), Some(IrType::Channel(_))) {
                report(format!(
                    "{position}: channel drain receiver is not a Channel"
                ));
            }
            if !matches!(inst.ty, IrType::List(_)) {
                report(format!("{position}: ChannelDrain must produce a List"));
            }
        }
    }
}

fn verify_atomic_receiver(
    _function: &Function,
    ty: Option<IrType>,
    position: &str,
    report: &mut impl FnMut(String),
) {
    if !matches!(ty, Some(IrType::Atomic(_))) {
        report(format!(
            "{position}: Atomic receiver is not a managed Atomic"
        ));
    }
}

fn verify_atomic_value(
    receiver: Option<IrType>,
    value: Option<IrType>,
    position: &str,
    report: &mut impl FnMut(String),
) {
    let expected = match receiver {
        Some(IrType::Atomic(kind)) => Some(kind.ir_type()),
        _ => None,
    };
    if value != expected {
        report(format!(
            "{position}: Atomic value type does not match receiver payload"
        ));
    }
}

fn verify_atomic_output(
    function: &Function,
    slot: SlotId,
    receiver: Option<IrType>,
    position: &str,
    report: &mut impl FnMut(String),
) {
    let expected = match receiver {
        Some(IrType::Atomic(kind)) => Some(kind.ir_type()),
        _ => None,
    };
    if function.slot(slot).map(|slot| slot.ty) != expected {
        report(format!(
            "{position}: Atomic output slot has the wrong scalar type"
        ));
    }
}

fn verify_atomic_order(
    type_of: impl Fn(&Operand) -> Option<IrType>,
    order: Option<Operand>,
    position: &str,
    report: &mut impl FnMut(String),
) {
    if let Some(order) = order
        && !matches!(
            type_of(&order),
            Some(IrType::Int(IntWidth::I32 | IntWidth::U32))
        )
    {
        report(format!(
            "{position}: AtomicOrder must be represented by an i32/u32 discriminant"
        ));
    }
}

fn verify_terminator(
    module: &Module,
    function: &Function,
    block: &Block,
    terminator: &Terminator,
    values: &HashMap<ValueId, (BlockId, IrType)>,
    report: &mut impl FnMut(String),
) {
    let position = format!("block {:?}, terminator", block.id);

    match terminator {
        // Nothing to check: a block nothing reaches transfers control nowhere.
        Terminator::Unreachable | Terminator::Cancel => {}

        Terminator::Return(value) => {
            let returned = match value {
                None => IrType::Void,
                Some(operand) => match values.get(&operand.0) {
                    Some((origin, ty)) => {
                        if matches!(ty, IrType::BorrowedView { .. }) {
                            report(format!("{position}: cannot return a BorrowedView"));
                        }
                        if *origin != block.id {
                            report(format!(
                                "{position}: returns value {:?} defined in another block",
                                operand.0
                            ));
                        }
                        *ty
                    }
                    None => {
                        report(format!(
                            "{position}: returns undefined value {:?}",
                            operand.0
                        ));
                        return;
                    }
                },
            };

            if returned != function.return_type {
                report(format!(
                    "{position}: returns {} in a function declared {}",
                    returned.as_str(),
                    function.return_type.as_str()
                ));
            }
        }

        Terminator::Branch { condition, .. } => match values.get(&condition.0) {
            None => report(format!(
                "{position}: branches on undefined value {:?}",
                condition.0
            )),
            Some((_, ty)) if *ty != IrType::Boolean => report(format!(
                "{position}: branches on {}, expected Boolean",
                ty.as_str()
            )),
            Some(_) => {}
        },

        Terminator::Jump(_) => {}

        Terminator::UnionDispatch {
            value,
            union_id,
            targets,
        } => match values.get(&value.0) {
            None => report(format!(
                "{position}: dispatches on undefined value {:?}",
                value.0
            )),
            Some((_, IrType::Union(representation))) => {
                match module.union_types.get(*union_id as usize) {
                    None => report(format!(
                        "{position}: dispatches with missing union layout {union_id}"
                    )),
                    Some(layout) if layout.representation_layout != Some(*representation) => {
                        report(format!(
                            "{position}: dispatch value does not match union layout {union_id}"
                        ))
                    }
                    Some(layout) if targets.len() != layout.alternatives.len() => report(format!(
                        "{position}: union dispatch has {} targets for {} alternatives",
                        targets.len(),
                        layout.alternatives.len()
                    )),
                    Some(_) => {}
                }
            }
            Some((_, ty)) => report(format!(
                "{position}: dispatches on {}, expected a union",
                ty.as_str()
            )),
        },
    }
}

fn expect(
    actual: IrType,
    expected: IrType,
    position: &str,
    what: &str,
    report: &mut impl FnMut(String),
) {
    if actual != expected {
        report(format!(
            "{position}: {what} declares type {} and should be {}",
            actual.as_str(),
            expected.as_str()
        ));
    }
}

fn verify_local_object_result(
    module: &Module,
    enum_id: u32,
    class_id: u32,
    position: &str,
    report: &mut impl FnMut(String),
) {
    let Some(layout) = module.enums.get(enum_id as usize) else {
        report(format!(
            "{position}: local result references missing enum layout {enum_id}"
        ));
        return;
    };
    let Some(field) = layout.variants.first().and_then(|fields| fields.first()) else {
        report(format!(
            "{position}: local result success variant has no object payload"
        ));
        return;
    };
    let expected = IrType::Object(class_id);
    if layout.fields.get(*field as usize).map(|field| field.ty) != Some(expected) {
        report(format!(
            "{position}: local result success payload must be object class {class_id}"
        ));
    }
}

/// Callable values are represented by a concrete closure layout, while a
/// function-typed field stores the canonical erased layout for its signature.
/// A captured lambda is compatible with that field when its call signature
/// matches; the boxed callable retains its concrete capture descriptor.
fn types_compatible(module: &Module, actual: IrType, expected: IrType) -> bool {
    if actual == expected {
        return true;
    }
    let (IrType::Callable(actual_id), IrType::Callable(expected_id)) = (actual, expected) else {
        return false;
    };
    let (Some(actual), Some(expected)) = (
        module.closures.get(actual_id as usize),
        module.closures.get(expected_id as usize),
    ) else {
        return false;
    };
    actual.params == expected.params && actual.returns == expected.returns
}

/// The type of field `index` of an object, if the operand is one and has it.
fn field_type(module: &Module, object: Option<IrType>, index: u32) -> Option<IrType> {
    match object? {
        IrType::Object(id) => module
            .objects
            .get(id as usize)?
            .fields
            .get(index as usize)
            .map(|f| f.ty),
        IrType::Value(id) => module
            .values
            .get(id as usize)?
            .fields
            .get(index as usize)
            .map(|f| f.ty),
        IrType::Enum(id) => module
            .enums
            .get(id as usize)?
            .fields
            .get(index as usize)
            .map(|f| f.ty),
        IrType::Pin(id) => field_type(module, Some(module.pin_types[id as usize]), index),
        _ => None,
    }
}

/// Operands an instruction reads.
fn operands_of(kind: &InstKind) -> Vec<Operand> {
    match kind {
        InstKind::BorrowViewStart { mutex, .. } => vec![*mutex],
        InstKind::BorrowViewEnd { view } | InstKind::BorrowViewLoad { view } => vec![*view],
        InstKind::BorrowViewFieldLoad { view, .. } => vec![*view],
        InstKind::BorrowViewFieldStore { view, value, .. } => vec![*view, *value],
        InstKind::BorrowViewStore { view, value } => vec![*view, *value],
        InstKind::MutexWithCall {
            mutex, callback, ..
        } => vec![*mutex, *callback],
        InstKind::MutexNew { payload, .. } => vec![*payload],
        InstKind::ProjectGlobalSet { value, .. } => vec![*value],
        InstKind::ProjectGlobalWatchSubscribe { channel, .. } => vec![*channel],
        InstKind::ProjectGlobalWatchNext { token, .. }
        | InstKind::ProjectGlobalWatchClose { token } => vec![*token],
        InstKind::ProjectGlobalInit { .. } | InstKind::ProjectGlobalGet { .. } => Vec::new(),
        InstKind::ConstInt(_)
        | InstKind::ConstBool(_)
        | InstKind::ConstString(_)
        | InstKind::ConstChar(_)
        | InstKind::MutexAcquire { .. }
        | InstKind::MutexRelease { .. }
        | InstKind::SafepointPoll => Vec::new(),
        InstKind::FatalError(message) => vec![*message],
        InstKind::Throw(exception) => vec![*exception],
        InstKind::SetSuppressed {
            exception,
            suppressed,
        } => vec![*exception, *suppressed],
        InstKind::StackTrace(exception) | InstKind::Suppressed(exception) => vec![*exception],
        InstKind::HasPendingException
        | InstKind::IsCancelled
        | InstKind::CancelShieldEnter
        | InstKind::CancelShieldExit
        | InstKind::TakePendingException => Vec::new(),
        InstKind::Load(_) => Vec::new(),
        InstKind::Store(_, operand) => vec![*operand],
        InstKind::AtomicNew { value, .. } => vec![*value],
        InstKind::AtomicLoad { object, order, .. } => {
            let mut operands = vec![*object];
            operands.extend(order.iter().copied());
            operands
        }
        InstKind::AtomicStore {
            object,
            value,
            order,
        }
        | InstKind::AtomicExchange {
            object,
            value,
            order,
            ..
        }
        | InstKind::AtomicFetchAdd {
            object,
            value,
            order,
            ..
        }
        | InstKind::AtomicFetchSub {
            object,
            value,
            order,
            ..
        } => {
            let mut operands = vec![*object, *value];
            operands.extend(order.iter().copied());
            operands
        }
        InstKind::AtomicCompareExchange {
            object,
            current,
            new_value,
            success_order,
            failure_order,
            ..
        } => vec![
            *object,
            *current,
            *new_value,
            *success_order,
            *failure_order,
        ],
        InstKind::Alloc(_) | InstKind::ObjectNew => Vec::new(),
        InstKind::ObjectSeal { object } => vec![*object],
        InstKind::DynamicObjectUnbox { value } => vec![*value],
        InstKind::ObjectConform { object, .. } => vec![*object],
        InstKind::BuildValue { fields, .. } => fields.clone(),
        InstKind::BuildEnum { fields, .. } => fields.clone(),
        InstKind::UnionInject { payload, .. } => vec![*payload],
        InstKind::UnionTag(value) | InstKind::UnionProject { value, .. } => vec![*value],
        InstKind::Discriminant(operand) => vec![*operand],
        InstKind::CheckedCast { object, .. } => vec![*object],
        InstKind::IsInstance { object, .. } => vec![*object],
        InstKind::Undefined => vec![],
        InstKind::Retype(operand) => vec![*operand],
        InstKind::IntCast(operand) => vec![*operand],
        InstKind::ConstFloat(_, _) | InstKind::ConstDecimal(_) => Vec::new(),
        InstKind::FloatCast(operand)
        | InstKind::IntToFloat(operand)
        | InstKind::FloatToInt(operand)
        | InstKind::IntToDecimal(operand)
        | InstKind::DecimalToInt(operand)
        | InstKind::DecimalToFloat(operand)
        | InstKind::FloatToDecimal(operand) => vec![*operand],
        InstKind::StringGraphemeOffset { string, index } => vec![*string, *index],
        InstKind::GraphemeLenAt { string, offset } => vec![*string, *offset],
        InstKind::GraphemeSlice {
            string,
            offset,
            len,
        } => vec![*string, *offset, *len],
        InstKind::Concat { left, right } => vec![*left, *right],
        InstKind::Repeat { string, count } => vec![*string, *count],
        InstKind::CallContract { object, args, .. } => {
            let mut operands = vec![*object];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::CallVirtual { object, args, .. } => {
            let mut operands = vec![*object];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::LoadField { object, .. } => vec![*object],
        InstKind::StoreField { object, value, .. } => vec![*object, *value],
        InstKind::DynamicPack { value } => vec![*value],
        InstKind::DynamicIsType { value, .. } | InstKind::DynamicUnbox { value, .. } => {
            vec![*value]
        }
        InstKind::ObjectSet { object, key, value } => vec![*object, *key, *value],
        InstKind::ObjectGet { object, key } => vec![*object, *key],
        InstKind::ObjectMerge {
            destination,
            source,
        } => vec![*destination, *source],
        InstKind::ObjectRest {
            object,
            excluded_keys,
        } => std::iter::once(*object)
            .chain(excluded_keys.iter().copied())
            .collect(),
        InstKind::JsonStringifyDynamic { value } => vec![*value],
        InstKind::JsonParseDynamic { document, .. } => vec![*document],
        InstKind::Unary { operand, .. } => vec![*operand],
        InstKind::Binary { left, right, .. } => vec![*left, *right],
        InstKind::CheckedArithmetic { left, right, .. } => vec![*left, *right],
        InstKind::Call { args, .. } => args.clone(),
        InstKind::ToString(operand) => vec![*operand],
        InstKind::Float128Format { value, spec } => vec![*value, *spec],
        InstKind::Println(operand)
        | InstKind::Print(operand)
        | InstKind::PrintlnStderr(operand)
        | InstKind::PrintStderr(operand) => vec![*operand],
        InstKind::FlushStdout { .. }
        | InstKind::FlushStderr { .. }
        | InstKind::EnvironmentListNames { .. }
        | InstKind::NetworkInterfacesSnapshot { .. } => Vec::new(),
        InstKind::StdinReadLineResult { .. } | InstKind::StdinReadChar { .. } => Vec::new(),
        InstKind::StdinReadBytes { max, .. } => vec![*max],
        InstKind::StdinReadExact { count, .. } => vec![*count],
        InstKind::EnvironmentGet { name, .. } | InstKind::EnvironmentContains { name, .. } => {
            vec![*name]
        }
        InstKind::FilesystemCall { args, .. } => args.clone(),
        InstKind::NetworkTcpConnect { endpoint, .. }
        | InstKind::NetworkDnsResolve { endpoint, .. }
        | InstKind::NetworkTlsConnect { endpoint, .. }
        | InstKind::NetworkTcpListen { endpoint, .. } => vec![*endpoint],
        InstKind::NetworkTlsConnectWithRoots {
            endpoint,
            roots_der,
            allow_tls12,
            ..
        } => vec![*endpoint, *roots_der, *allow_tls12],
        InstKind::NetworkTlsConnectWithAlpn {
            endpoint,
            roots_der,
            allow_tls12,
            protocols,
            ..
        } => vec![*endpoint, *roots_der, *allow_tls12, *protocols],
        InstKind::NetworkTlsUpgradeTcp {
            tcp_handle,
            endpoint,
            roots_der,
            allow_tls12,
            protocols,
            ..
        } => vec![*tcp_handle, *endpoint, *roots_der, *allow_tls12, *protocols],
        InstKind::NetworkTlsConnectWithClientIdentity {
            endpoint,
            roots_der,
            certificate_chain_der,
            private_key_der,
            allow_tls12,
            protocols,
            ..
        } => vec![
            *endpoint,
            *roots_der,
            *certificate_chain_der,
            *private_key_der,
            *allow_tls12,
            *protocols,
        ],
        InstKind::NetworkUdpConnect { endpoint, .. }
        | InstKind::NetworkUdpBind { endpoint, .. } => vec![*endpoint],
        InstKind::NetworkLocalConnect { endpoint, .. } => vec![*endpoint],
        InstKind::NetworkLocalBind {
            endpoint,
            remove_stale,
            ..
        } => {
            vec![*endpoint, *remove_stale]
        }
        InstKind::NetworkLocalAccept { handle, .. }
        | InstKind::NetworkLocalClose { handle, .. }
        | InstKind::NetworkLocalShutdown { handle, .. }
        | InstKind::NetworkLocalIsClosed { handle, .. } => vec![*handle],
        InstKind::NetworkLocalRead {
            handle, max_bytes, ..
        } => vec![*handle, *max_bytes],
        InstKind::NetworkLocalWrite { handle, bytes, .. } => vec![*handle, *bytes],
        InstKind::NetworkUdpSend { handle, bytes, .. } => vec![*handle, *bytes],
        InstKind::NetworkUdpSendTo {
            handle,
            bytes,
            endpoint,
            ..
        } => {
            vec![*handle, *bytes, *endpoint]
        }
        InstKind::NetworkUdpSendBroadcast {
            handle,
            bytes,
            endpoint,
            ..
        }
        | InstKind::NetworkUdpSendMulticast {
            handle,
            bytes,
            endpoint,
            ..
        } => {
            vec![*handle, *bytes, *endpoint]
        }
        InstKind::NetworkUdpJoinMulticast { handle, group, .. }
        | InstKind::NetworkUdpLeaveMulticast { handle, group, .. } => vec![*handle, *group],
        InstKind::NetworkUdpReceive {
            handle, max_bytes, ..
        } => vec![*handle, *max_bytes],
        InstKind::NetworkUdpClose { handle, .. }
        | InstKind::NetworkUdpLocalAddress { handle, .. }
        | InstKind::NetworkUdpIsClosed { handle } => {
            vec![*handle]
        }
        InstKind::NetworkTlsListen {
            endpoint,
            certificate_chain_der,
            private_key_der,
            allow_tls12,
            ..
        } => vec![
            *endpoint,
            *certificate_chain_der,
            *private_key_der,
            *allow_tls12,
        ],
        InstKind::NetworkTlsListenWithClientAuth {
            endpoint,
            certificate_chain_der,
            private_key_der,
            client_roots_der,
            allow_tls12,
            ..
        } => vec![
            *endpoint,
            *certificate_chain_der,
            *private_key_der,
            *client_roots_der,
            *allow_tls12,
        ],
        InstKind::NetworkIpAddressParse { value, .. } => vec![*value],
        InstKind::NetworkTcpOriginNormalize { host, port, .. } => vec![*host, *port],
        InstKind::NetworkTcpRead {
            handle, max_bytes, ..
        }
        | InstKind::NetworkTlsRead {
            handle, max_bytes, ..
        } => vec![*handle, *max_bytes],
        InstKind::NetworkTcpWrite { handle, bytes, .. }
        | InstKind::NetworkTcpWriteAll { handle, bytes, .. }
        | InstKind::NetworkTlsWriteAll { handle, bytes, .. } => vec![*handle, *bytes],
        InstKind::NetworkTcpShutdown { handle, .. }
        | InstKind::NetworkTcpClose { handle, .. }
        | InstKind::NetworkTlsShutdown { handle, .. }
        | InstKind::NetworkTlsClose { handle, .. }
        | InstKind::NetworkTlsIsClosed { handle }
        | InstKind::NetworkTlsListenerIsClosed { handle }
        | InstKind::NetworkTlsPeerAddress { handle, .. }
        | InstKind::NetworkTlsNegotiatedAlpn { handle, .. }
        | InstKind::NetworkTlsPeerCertificateSha256 { handle, .. }
        | InstKind::NetworkTcpIsClosed { handle }
        | InstKind::NetworkTcpPeerAddress { handle, .. } => vec![*handle],
        InstKind::NetworkTcpAccept {
            handle,
            shutdown_control,
            ..
        }
        | InstKind::NetworkTlsAccept {
            handle,
            shutdown_control,
            ..
        } => std::iter::once(*handle)
            .chain(shutdown_control.iter().copied())
            .collect(),
        InstKind::NetworkTcpCloseListener { handle, .. }
        | InstKind::NetworkTcpListenerIsClosed { handle }
        | InstKind::NetworkTlsCloseListener { handle, .. } => vec![*handle],
        InstKind::HttpGet { url, .. } => vec![*url],
        InstKind::HttpRequest {
            method,
            url,
            body,
            headers,
            validate_json,
            ..
        } => vec![*method, *url, *body, *headers, *validate_json],
        InstKind::HttpClientCreate { max_origins, .. } => vec![*max_origins],
        InstKind::HttpClientCreateWithOptions {
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            ..
        } => vec![
            *max_origins,
            *connect_timeout_ms,
            *response_headers_timeout_ms,
            *response_idle_timeout_ms,
            *protocol_code,
            *protocol_code,
        ],
        InstKind::HttpClientCreateWithTrustRoots {
            roots,
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            ..
        } => vec![
            *roots,
            *max_origins,
            *connect_timeout_ms,
            *response_headers_timeout_ms,
            *response_idle_timeout_ms,
            *protocol_code,
        ],
        InstKind::HttpClientCreateWithProxy {
            proxy_url,
            proxy_credentials,
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            ..
        } => vec![
            *proxy_url,
            *proxy_credentials,
            *max_origins,
            *connect_timeout_ms,
            *response_headers_timeout_ms,
            *response_idle_timeout_ms,
            *protocol_code,
        ],
        InstKind::HttpClientCreateWithProxyAndTrustRoots {
            proxy_url,
            proxy_credentials,
            roots,
            max_origins,
            connect_timeout_ms,
            response_headers_timeout_ms,
            response_idle_timeout_ms,
            protocol_code,
            ..
        } => vec![
            *proxy_url,
            *proxy_credentials,
            *roots,
            *max_origins,
            *connect_timeout_ms,
            *response_headers_timeout_ms,
            *response_idle_timeout_ms,
            *protocol_code,
        ],
        InstKind::HttpClientRequest {
            handle,
            method,
            url,
            body,
            headers,
            validate_json,
            ..
        } => {
            vec![*handle, *method, *url, *body, *headers, *validate_json]
        }
        InstKind::HttpClientRequestStream {
            handle,
            method,
            url,
            headers,
            callback,
            ..
        } => {
            vec![*handle, *method, *url, *headers, *callback]
        }
        InstKind::HttpClientAction { handle, .. } => vec![*handle],
        InstKind::HttpWebSocketConnect {
            url,
            origin,
            subprotocols,
            ..
        } => {
            vec![*url, *origin, *subprotocols]
        }
        InstKind::HttpWebSocketConnectWithTrustRoots {
            url,
            origin,
            subprotocols,
            roots_der,
            allow_tls12,
            ..
        } => vec![*url, *origin, *subprotocols, *roots_der, *allow_tls12],
        InstKind::HttpWebSocketSend { handle, record, .. } => vec![*handle, *record],
        InstKind::HttpWebSocketReceive { handle, .. }
        | InstKind::HttpWebSocketClose { handle, .. } => vec![*handle],
        InstKind::HttpWebSocketCloseWith {
            handle,
            code,
            reason,
            ..
        } => vec![*handle, *code, *reason],
        InstKind::HttpRequestSecret {
            method,
            url,
            body,
            headers,
            header_name,
            secret_value,
            ..
        } => vec![*method, *url, *body, *headers, *header_name, *secret_value],
        InstKind::HttpHead { url, .. } => vec![*url],
        InstKind::HttpQuery { url, query, .. } => vec![*url, *query],
        InstKind::HttpQueryWithContentType {
            url,
            query,
            content_type,
            ..
        } => vec![*url, *query, *content_type],
        InstKind::HttpPost { url, body, .. } => vec![*url, *body],
        InstKind::HttpPostJson { url, body, .. } => vec![*url, *body],
        InstKind::HttpPostBytes { url, body, .. } => vec![*url, *body],
        InstKind::HttpGetStream { url, headers, .. } => vec![*url, *headers],
        InstKind::HttpResponseToStream {
            status,
            body,
            headers,
            ..
        } => vec![*status, *body, *headers],
        InstKind::HttpRequestStream {
            method,
            url,
            headers,
            callback,
            ..
        } => vec![*method, *url, *headers, *callback],
        InstKind::HttpRegisterServerResponseProducer { callback, .. } => vec![*callback],
        InstKind::HttpResponseReadChunk {
            handle, max_bytes, ..
        } => vec![*handle, *max_bytes],
        InstKind::HttpResponseReadAll { handle, .. } => vec![*handle],
        InstKind::HttpResponseReadSseEvent { handle, .. } => vec![*handle],
        InstKind::HttpResponseSseHeartbeatCount { handle } => vec![*handle],
        InstKind::HttpResponseStreamClose { handle } => vec![*handle],
        InstKind::HttpUrlNormalize { value, .. } => vec![*value],
        InstKind::HttpUrlResolve {
            base, reference, ..
        } => vec![*base, *reference],
        InstKind::HttpResponseText { body, .. } => vec![*body],
        InstKind::HttpResponseHeaderValues { headers, name, .. } => vec![*headers, *name],
        InstKind::HttpRouteMatch { pattern, path } => vec![*pattern, *path],
        InstKind::HttpFormParse {
            body, content_type, ..
        } => vec![*body, *content_type],
        InstKind::HttpMultipartParse {
            body, content_type, ..
        } => vec![*body, *content_type],
        InstKind::HttpUploadRead {
            handle,
            offset,
            max_bytes,
            ..
        } => vec![*handle, *offset, *max_bytes],
        InstKind::HttpUploadClose { handle } => vec![*handle],
        InstKind::HttpServerControlCreate => vec![],
        InstKind::HttpServerControlRequestShutdown { handle } => vec![*handle],
        InstKind::HttpServerServeOnce {
            stream_handle,
            max_requests,
            callback,
            ..
        } => vec![*stream_handle, *max_requests, *callback],
        InstKind::HttpServerServeHttp3 {
            endpoint,
            certificate_chain_der,
            private_key_der,
            max_requests,
            max_connections,
            callback,
            ..
        } => vec![
            *endpoint,
            *certificate_chain_der,
            *private_key_der,
            *max_requests,
            *max_connections,
            *callback,
        ],
        InstKind::HttpServerWebSocketServeOnce {
            stream_handle,
            path,
            allowed_origins,
            allow_missing_origin,
            protocols,
            max_messages,
            callback,
            ..
        } => vec![
            *stream_handle,
            *path,
            *allowed_origins,
            *allow_missing_origin,
            *protocols,
            *max_messages,
            *callback,
        ],
        InstKind::HttpServerWebSocketServeHttp3Once {
            endpoint,
            certificate_chain_der,
            private_key_der,
            path,
            allowed_origins,
            allow_missing_origin,
            protocols,
            max_messages,
            idle_timeout_ms,
            callback,
            ..
        } => vec![
            *endpoint,
            *certificate_chain_der,
            *private_key_der,
            *path,
            *allowed_origins,
            *allow_missing_origin,
            *protocols,
            *max_messages,
            *idle_timeout_ms,
            *callback,
        ],
        InstKind::NullValue(_) => Vec::new(),
        InstKind::Wrap { value, .. } => vec![*value],
        InstKind::IsNull(operand) | InstKind::Unwrap(operand) => vec![*operand],
        InstKind::MakeClosure { captures, .. } => captures.clone(),
        InstKind::CallClosure { callee, args, .. } => {
            let mut operands = vec![*callee];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::PointerFromSlot(_) => Vec::new(),
        InstKind::PointerFromField { object, .. } => vec![*object],
        InstKind::WordCast(operand) => vec![*operand],
        InstKind::PointerRead(operand)
        | InstKind::PointerCast(operand)
        | InstKind::PointerIsNull(operand) => vec![*operand],
        InstKind::PointerWrite { pointer, value } => vec![*pointer, *value],
        InstKind::PointerOffset { pointer, amount }
        | InstKind::PointerOffsetBytes { pointer, amount } => {
            vec![*pointer, *amount]
        }
        InstKind::WeakFrom(operand)
        | InstKind::WeakUpgrade(operand)
        | InstKind::WeakIsAlive(operand)
        | InstKind::Clone(operand) => vec![*operand],
        InstKind::JournalBegin => Vec::new(),
        InstKind::JournalRecordSlot { journal, .. } => vec![*journal],
        InstKind::JournalRecordField {
            journal, object, ..
        } => vec![*journal, *object],
        InstKind::JournalCommit(journal) | InstKind::JournalRollback(journal) => vec![*journal],
        InstKind::NativeSliceValidate {
            pointer, length, ..
        } => vec![*pointer, *length],
        InstKind::NativeSliceValue { pointer, length } => vec![*pointer, *length],
        InstKind::NativeSliceLength(operand) => vec![*operand],
        InstKind::ArrayLength(operand) | InstKind::ListLength(operand) => vec![*operand],
        InstKind::NativeSliceLoad { receiver, index } => vec![*receiver, *index],
        InstKind::NativeSliceStore {
            receiver,
            index,
            value,
        } => vec![*receiver, *index, *value],
        InstKind::MakeCallable { captures, .. } => captures.clone(),
        InstKind::CallCallable { callable, args } => {
            let mut operands = vec![*callable];
            operands.extend(args.iter().copied());
            operands
        }
        InstKind::ThreadRun { .. } => Vec::new(),
        InstKind::ResourceTransfer { source } => vec![*source],
        InstKind::DependentFrom { base, field_ptr } => vec![*base, *field_ptr],
        InstKind::PinObject { object } | InstKind::UnpinObject { object } => vec![*object],
        InstKind::ArrayNew { capacity, .. } => vec![*capacity],
        InstKind::ListNew { .. }
        | InstKind::DequeNew { .. }
        | InstKind::PriorityQueueNew { .. }
        | InstKind::MapNew { .. }
        | InstKind::SetNew { .. } => Vec::new(),
        InstKind::ArrayListLoad { receiver, index } => vec![*receiver, *index],
        InstKind::DequeLength(receiver)
        | InstKind::DequeIsEmpty(receiver)
        | InstKind::DequeVersion(receiver)
        | InstKind::DequeFront(receiver)
        | InstKind::DequeBack(receiver)
        | InstKind::DequePopFront(receiver)
        | InstKind::DequePopBack(receiver)
        | InstKind::DequeClear(receiver) => vec![*receiver],
        InstKind::PriorityQueueLength(receiver)
        | InstKind::PriorityQueueIsEmpty(receiver)
        | InstKind::PriorityQueueVersion(receiver)
        | InstKind::PriorityQueuePeek(receiver)
        | InstKind::PriorityQueueDequeue(receiver) => vec![*receiver],
        InstKind::PriorityQueueEnqueue { receiver, value } => vec![*receiver, *value],
        InstKind::DequePushFront { receiver, value }
        | InstKind::DequePushBack { receiver, value } => vec![*receiver, *value],
        InstKind::ArrayListStore {
            receiver,
            index,
            value,
        } => vec![*receiver, *index, *value],
        InstKind::ListAdd { receiver, value } => vec![*receiver, *value],
        InstKind::ListInsert {
            receiver,
            index,
            value,
        } => vec![*receiver, *index, *value],
        InstKind::ListRemove { receiver, index } => vec![*receiver, *index],
        InstKind::ListRemoveValue { receiver, value } => vec![*receiver, *value],
        InstKind::MapLength(receiver)
        | InstKind::MapVersion(receiver)
        | InstKind::MapKeys(receiver)
        | InstKind::MapSortedKeys(receiver)
        | InstKind::MapValues(receiver)
        | InstKind::MapEntries(receiver)
        | InstKind::MapIsEmpty(receiver)
        | InstKind::MapClear(receiver)
        | InstKind::SetLength(receiver)
        | InstKind::SetValues(receiver)
        | InstKind::SetSortedElements(receiver)
        | InstKind::SetIsEmpty(receiver)
        | InstKind::SetClear(receiver)
        | InstKind::ArrayClone { receiver }
        | InstKind::ListClone { receiver } => vec![*receiver],
        InstKind::MapSet {
            receiver,
            key,
            value,
        } => vec![*receiver, *key, *value],
        InstKind::MapContainsKey { receiver, key } => vec![*receiver, *key],
        InstKind::MapGet { receiver, key } => vec![*receiver, *key],
        InstKind::MapRemove { receiver, key } => vec![*receiver, *key],
        InstKind::SetAdd { receiver, value }
        | InstKind::SetContains { receiver, value }
        | InstKind::SetRemove { receiver, value } => {
            vec![*receiver, *value]
        }
        InstKind::SetOperation { left, right, .. } => vec![*left, *right],
        InstKind::ArraySlice {
            receiver,
            start,
            end,
            step,
        } => vec![*receiver, *start, *end, *step],
        InstKind::ScopeEnter | InstKind::ScopeEnterCollectAll => Vec::new(),
        InstKind::ScopeExit { scope }
        | InstKind::ScopeExitCollectAll { scope }
        | InstKind::ScopeCancel { scope } => vec![*scope],
        InstKind::ScopeOutcomeWaitTake { scope, .. } => vec![*scope],
        InstKind::BranchStart {
            body, scope, delay, ..
        } => {
            let mut operands = vec![*body, *scope];
            operands.extend(delay.iter().copied());
            operands
        }
        InstKind::BackgroundStart { body, .. } => vec![*body],
        InstKind::ServiceStart { body, .. } => vec![*body],
        InstKind::ServiceReady { context, .. } => vec![*context],
        InstKind::JobWait { job, .. }
        | InstKind::JobOutcomeWaitTake { job, .. }
        | InstKind::JobOutcomeTake { job, .. }
        | InstKind::JobDone { job }
        | InstKind::JobCancel { job } => {
            vec![*job]
        }
        InstKind::JobCancelWithCleanupFailure { job } => vec![*job],
        InstKind::JobWaitAny { jobs } | InstKind::JobWaitAnyWithTimer { jobs } => jobs.clone(),
        InstKind::JobResultLoad { box_pointer, .. } => vec![*box_pointer],
        InstKind::TimerSleep { nanos } => vec![*nanos],
        InstKind::AppSetShutdownGrace { nanos } => vec![*nanos],
        InstKind::AppSetBackgroundError { callable, .. } => vec![*callable],
        InstKind::ChannelNew {
            capacity, limit, ..
        } => vec![*capacity, *limit],
        InstKind::ChannelSend { channel, value } | InstKind::ChannelTrySend { channel, value } => {
            vec![*channel, *value]
        }
        InstKind::ChannelRecv { channel, .. }
        | InstKind::ChannelTryRecv { channel, .. }
        | InstKind::ChannelClose { channel }
        | InstKind::ChannelLength(channel)
        | InstKind::ChannelIsClosed(channel)
        | InstKind::ChannelCapacity(channel) => vec![*channel],
        InstKind::ChannelDrain { channel, .. } => vec![*channel],
        InstKind::ParallelForStart {
            body,
            count,
            budget_a,
            budget_b,
            ..
        } => vec![*body, *count, *budget_a, *budget_b],
    }
}
