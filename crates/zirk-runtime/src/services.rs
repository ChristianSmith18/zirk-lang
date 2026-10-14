//! Bounded runtime-only service lifecycle and executor callback context.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

/// Executor-owned service callback context. Readiness is the only state
/// transition exposed to a callback; cancellation remains cooperative through
/// the executor's normal task safe points.
#[derive(Debug, Clone, Copy)]
pub struct ServiceContext {
    id: ServiceId,
}

impl ServiceContext {
    pub(crate) fn new(id: ServiceId) -> Self {
        Self { id }
    }

    /// Publishes successful startup for this service.
    pub fn ready(self) -> Result<ServiceState, ServiceError> {
        crate::executor::service_ready_current(self.id)
    }

    /// Stable runtime identity, useful to host-side callback implementations.
    pub fn id(self) -> ServiceId {
        self.id
    }
}

/// Default maximum number of services owned by one supervisor.
pub const DEFAULT_SERVICE_CAPACITY: usize = 64;

/// Stable identity assigned in registration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(pub u64);

/// Lifecycle shared by the supervisor and its registered services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Created,
    Starting,
    Ready,
    Draining,
    Stopped,
}

/// Operation that was rejected by the service lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceOperation {
    Register,
    BeginStartup,
    BeginService,
    ReportReady,
    FailStartup,
    FailAdmission,
    CancelStartup,
    FailRuntime,
    RequestShutdown,
    NotifyStopped,
}

/// Explicit errors for invalid requests and bounded admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    InvalidCapacity,
    CapacityReached,
    TaskCapacityReached,
    IdExhausted,
    UnknownService(ServiceId),
    UnknownDependency(ServiceId),
    DuplicateDependency(ServiceId),
    DependencyNotReady(ServiceId),
    InvalidTransition {
        operation: ServiceOperation,
        state: ServiceState,
    },
    ServiceNotStarting(ServiceId),
    ServiceNotReady(ServiceId),
    WrongExecutorTask,
}

/// A failure retained for final application diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceFailure {
    pub service: ServiceId,
    pub name: String,
    pub during_startup: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
struct ServiceEntry {
    id: ServiceId,
    name: String,
    required: bool,
    dependencies: Vec<ServiceId>,
    state: ServiceState,
}

struct State {
    lifecycle: ServiceState,
    entries: Vec<ServiceEntry>,
    startup_order: Vec<ServiceId>,
    failures: VecDeque<ServiceFailure>,
}

/// Bounded application service lifecycle supervisor.
///
/// Services receive monotonically increasing IDs in registration order.
/// Startup order is the order of successful [`begin_service`](Self::begin_service)
/// calls; shutdown returns the reverse of that order. Startup failure and
/// startup cancellation fail closed and drain every service that began,
/// including optional services. After readiness, any service failure requests
/// application shutdown. There are no implicit retries.
pub struct ServiceSupervisor {
    capacity: usize,
    state: Mutex<State>,
}

impl ServiceSupervisor {
    /// Creates a supervisor with the conservative default finite capacity.
    pub fn new() -> Self {
        Self::build(DEFAULT_SERVICE_CAPACITY)
    }

    /// Creates a supervisor with an explicit finite service capacity.
    pub fn with_capacity(capacity: usize) -> Result<Self, ServiceError> {
        if capacity == 0 {
            return Err(ServiceError::InvalidCapacity);
        }
        Ok(Self::build(capacity))
    }

    fn build(capacity: usize) -> Self {
        Self {
            capacity,
            state: Mutex::new(State {
                lifecycle: ServiceState::Created,
                entries: Vec::new(),
                startup_order: Vec::new(),
                failures: VecDeque::new(),
            }),
        }
    }

    /// Registers a service before startup, returning its deterministic ID.
    pub fn register(
        &self,
        name: impl Into<String>,
        required: bool,
    ) -> Result<ServiceId, ServiceError> {
        self.register_with_dependencies(name, required, &[])
    }

    /// Registers a service that may start only after every dependency is
    /// ready. Dependencies must refer to distinct services already registered
    /// with this supervisor. Since edges can only point to earlier IDs, the
    /// dependency graph is acyclic by construction.
    pub fn register_with_dependencies(
        &self,
        name: impl Into<String>,
        required: bool,
        dependencies: &[ServiceId],
    ) -> Result<ServiceId, ServiceError> {
        let mut state = self.lock();
        if !matches!(
            state.lifecycle,
            ServiceState::Created | ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(invalid(ServiceOperation::Register, state.lifecycle));
        }
        if state.entries.len() >= self.capacity {
            return Err(ServiceError::CapacityReached);
        }
        let id =
            ServiceId(u64::try_from(state.entries.len()).map_err(|_| ServiceError::IdExhausted)?);
        let mut unique_dependencies = Vec::with_capacity(dependencies.len());
        for dependency in dependencies {
            if unique_dependencies.contains(dependency) {
                return Err(ServiceError::DuplicateDependency(*dependency));
            }
            if find_service(&state, *dependency).is_err() {
                return Err(ServiceError::UnknownDependency(*dependency));
            }
            unique_dependencies.push(*dependency);
        }
        state.entries.push(ServiceEntry {
            id,
            name: name.into(),
            required,
            dependencies: unique_dependencies,
            state: ServiceState::Created,
        });
        Ok(id)
    }

    /// Moves the supervisor to `Starting`. With no required services,
    /// readiness is immediate because the required set is empty.
    pub fn begin_startup(&self) -> Result<ServiceState, ServiceError> {
        let mut state = self.lock();
        if state.lifecycle != ServiceState::Created {
            return Err(invalid(ServiceOperation::BeginStartup, state.lifecycle));
        }
        state.lifecycle = ServiceState::Starting;
        if state
            .entries
            .iter()
            .filter(|service| service.required)
            .all(|service| service.state == ServiceState::Ready)
        {
            state.lifecycle = ServiceState::Ready;
        }
        Ok(state.lifecycle)
    }

    /// Records that a service has begun. The call order defines deterministic
    /// startup order and therefore reverse shutdown order.
    pub fn begin_service(&self, id: ServiceId) -> Result<(), ServiceError> {
        let mut state = self.lock();
        if !matches!(
            state.lifecycle,
            ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(invalid(ServiceOperation::BeginService, state.lifecycle));
        }
        let index = find_service(&state, id)?;
        if state.entries[index].state != ServiceState::Created {
            return Err(invalid(
                ServiceOperation::BeginService,
                state.entries[index].state,
            ));
        }
        if let Some(dependency) = state.entries[index].dependencies.iter().find(|dependency| {
            state
                .entries
                .iter()
                .find(|service| service.id == **dependency)
                .is_none_or(|service| service.state != ServiceState::Ready)
        }) {
            return Err(ServiceError::DependencyNotReady(*dependency));
        }
        state.entries[index].state = ServiceState::Starting;
        state.startup_order.push(id);
        Ok(())
    }

    /// Publishes one service as ready and publishes supervisor readiness only
    /// after every required service is ready.
    pub fn report_ready(&self, id: ServiceId) -> Result<ServiceState, ServiceError> {
        let mut state = self.lock();
        if !matches!(
            state.lifecycle,
            ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(invalid(ServiceOperation::ReportReady, state.lifecycle));
        }
        let index = find_service(&state, id)?;
        if state.entries[index].state != ServiceState::Starting {
            return Err(ServiceError::ServiceNotStarting(id));
        }
        state.entries[index].state = ServiceState::Ready;
        if state
            .entries
            .iter()
            .filter(|service| service.required)
            .all(|service| service.state == ServiceState::Ready)
        {
            state.lifecycle = ServiceState::Ready;
        }
        Ok(state.lifecycle)
    }

    /// Records a startup failure, enters drain, and returns services to stop
    /// in reverse startup order. Any startup failure is fatal, including an
    /// optional service failure, to avoid silently running a partial app.
    pub fn fail_startup(
        &self,
        id: ServiceId,
        message: impl Into<String>,
    ) -> Result<Vec<ServiceId>, ServiceError> {
        let mut state = self.lock();
        if !matches!(
            state.lifecycle,
            ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(invalid(ServiceOperation::FailStartup, state.lifecycle));
        }
        let index = find_service(&state, id)?;
        if state.entries[index].state != ServiceState::Starting {
            return Err(ServiceError::ServiceNotStarting(id));
        }
        record_failure(&mut state, index, true, message.into());
        // Startup may have acquired resources before failing; keep this
        // service in the reverse-order cleanup set instead of treating it as
        // fully stopped merely because readiness was never reached.
        Ok(enter_draining(&mut state))
    }

    /// Fails startup when a registered callback cannot obtain an executor
    /// task slot. The callback never ran, so it is deliberately excluded from
    /// actual-start order and therefore has no synthetic cleanup task.
    pub fn fail_admission(
        &self,
        id: ServiceId,
        message: impl Into<String>,
    ) -> Result<Vec<ServiceId>, ServiceError> {
        let mut state = self.lock();
        if !matches!(
            state.lifecycle,
            ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(invalid(ServiceOperation::FailAdmission, state.lifecycle));
        }
        let index = find_service(&state, id)?;
        if state.entries[index].state != ServiceState::Created {
            return Err(invalid(
                ServiceOperation::FailAdmission,
                state.entries[index].state,
            ));
        }
        record_failure(&mut state, index, true, message.into());
        Ok(enter_draining(&mut state))
    }

    /// Cancels startup and returns started services to stop in reverse order.
    pub fn cancel_startup(&self) -> Result<Vec<ServiceId>, ServiceError> {
        let mut state = self.lock();
        if state.lifecycle != ServiceState::Starting {
            return Err(invalid(ServiceOperation::CancelStartup, state.lifecycle));
        }
        Ok(enter_draining(&mut state))
    }

    /// Records a failure after readiness, requests application shutdown, and
    /// returns the remaining services in reverse startup order.
    pub fn fail_runtime(
        &self,
        id: ServiceId,
        message: impl Into<String>,
    ) -> Result<Vec<ServiceId>, ServiceError> {
        let mut state = self.lock();
        if state.lifecycle != ServiceState::Ready {
            return Err(invalid(ServiceOperation::FailRuntime, state.lifecycle));
        }
        let index = find_service(&state, id)?;
        if !matches!(
            state.entries[index].state,
            ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(ServiceError::ServiceNotReady(id));
        }
        record_failure(&mut state, index, false, message.into());
        // A runtime failure does not prove that all service-owned resources
        // have been released; include the failed service in shutdown cleanup.
        Ok(enter_draining(&mut state))
    }

    /// Requests shutdown from startup or ready state and returns services to
    /// stop in reverse startup order. Unstarted services become stopped.
    pub fn request_shutdown(&self) -> Result<Vec<ServiceId>, ServiceError> {
        let mut state = self.lock();
        if !matches!(
            state.lifecycle,
            ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(invalid(ServiceOperation::RequestShutdown, state.lifecycle));
        }
        Ok(enter_draining(&mut state))
    }

    /// Idempotently acknowledges terminal service completion. The supervisor
    /// becomes `Stopped` after every started service has acknowledged stop.
    pub fn notify_stopped(&self, id: ServiceId) -> Result<ServiceState, ServiceError> {
        let mut state = self.lock();
        let index = find_service(&state, id)?;
        if state.entries[index].state == ServiceState::Stopped {
            return Ok(state.lifecycle);
        }
        if state.lifecycle != ServiceState::Draining
            || state.entries[index].state != ServiceState::Draining
        {
            return Err(invalid(ServiceOperation::NotifyStopped, state.lifecycle));
        }
        state.entries[index].state = ServiceState::Stopped;
        if state
            .entries
            .iter()
            .all(|service| service.state == ServiceState::Stopped)
        {
            state.lifecycle = ServiceState::Stopped;
        }
        Ok(state.lifecycle)
    }

    /// Returns the current supervisor lifecycle.
    pub fn lifecycle(&self) -> ServiceState {
        self.lock().lifecycle
    }

    /// Returns a service's current lifecycle, if the ID is registered.
    pub fn service_state(&self, id: ServiceId) -> Result<ServiceState, ServiceError> {
        let state = self.lock();
        Ok(state.entries[find_service(&state, id)?].state)
    }

    /// Returns registered services in deterministic registration order.
    pub fn services(&self) -> Vec<(ServiceId, String, bool, ServiceState)> {
        self.lock()
            .entries
            .iter()
            .map(|service| {
                (
                    service.id,
                    service.name.clone(),
                    service.required,
                    service.state,
                )
            })
            .collect()
    }

    /// Returns failures in occurrence order. At most `capacity` failures can
    /// exist because each service can fail terminally only once.
    pub fn failures(&self) -> Vec<ServiceFailure> {
        self.lock().failures.iter().cloned().collect()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for ServiceSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

fn invalid(operation: ServiceOperation, state: ServiceState) -> ServiceError {
    ServiceError::InvalidTransition { operation, state }
}

fn find_service(state: &State, id: ServiceId) -> Result<usize, ServiceError> {
    state
        .entries
        .iter()
        .position(|service| service.id == id)
        .ok_or(ServiceError::UnknownService(id))
}

fn record_failure(state: &mut State, index: usize, during_startup: bool, message: String) {
    let service = &state.entries[index];
    state.failures.push_back(ServiceFailure {
        service: service.id,
        name: service.name.clone(),
        during_startup,
        message,
    });
}

fn enter_draining(state: &mut State) -> Vec<ServiceId> {
    state.lifecycle = ServiceState::Draining;
    for service in &mut state.entries {
        if service.state == ServiceState::Created {
            service.state = ServiceState::Stopped;
        }
    }
    let mut stop_order = Vec::new();
    for id in state.startup_order.iter().rev().copied() {
        if let Some(service) = state.entries.iter_mut().find(|service| service.id == id)
            && (service.state == ServiceState::Starting || service.state == ServiceState::Ready)
        {
            service.state = ServiceState::Draining;
            stop_order.push(id);
        }
    }
    if state
        .entries
        .iter()
        .all(|service| service.state == ServiceState::Stopped)
    {
        state.lifecycle = ServiceState::Stopped;
    }
    stop_order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(required: bool) -> (ServiceSupervisor, ServiceId, ServiceId) {
        let supervisor = ServiceSupervisor::with_capacity(3).unwrap();
        let first = supervisor.register("first", required).unwrap();
        let second = supervisor.register("second", required).unwrap();
        supervisor.begin_startup().unwrap();
        supervisor.begin_service(first).unwrap();
        supervisor.begin_service(second).unwrap();
        (supervisor, first, second)
    }

    #[test]
    fn admission_is_bounded_and_registration_order_is_stable() {
        let supervisor = ServiceSupervisor::with_capacity(2).unwrap();
        assert!(matches!(
            ServiceSupervisor::with_capacity(0),
            Err(ServiceError::InvalidCapacity)
        ));
        let a = supervisor.register("a", true).unwrap();
        let b = supervisor.register("b", false).unwrap();
        assert_eq!(a, ServiceId(0));
        assert_eq!(b, ServiceId(1));
        assert_eq!(
            supervisor.register("c", false),
            Err(ServiceError::CapacityReached)
        );
        assert_eq!(
            supervisor.services(),
            vec![
                (a, "a".to_owned(), true, ServiceState::Created),
                (b, "b".to_owned(), false, ServiceState::Created),
            ]
        );
    }

    #[test]
    fn readiness_waits_for_every_required_service() {
        let (supervisor, first, second) = started(true);
        assert_eq!(
            supervisor.report_ready(second).unwrap(),
            ServiceState::Starting
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Starting);
        assert_eq!(supervisor.report_ready(first).unwrap(), ServiceState::Ready);
        assert_eq!(supervisor.lifecycle(), ServiceState::Ready);
    }

    #[test]
    fn dependent_service_waits_until_its_prerequisite_is_ready() {
        let supervisor = ServiceSupervisor::with_capacity(3).unwrap();
        let database = supervisor.register("database", true).unwrap();
        let api = supervisor
            .register_with_dependencies("api", true, &[database])
            .unwrap();
        let frontend = supervisor
            .register_with_dependencies("frontend", false, &[api])
            .unwrap();

        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));
        assert_eq!(
            supervisor.begin_service(api),
            Err(ServiceError::DependencyNotReady(database))
        );
        assert_eq!(supervisor.begin_service(database), Ok(()));
        assert_eq!(
            supervisor.report_ready(database),
            Ok(ServiceState::Starting)
        );

        assert_eq!(supervisor.begin_service(api), Ok(()));
        assert_eq!(
            supervisor.begin_service(frontend),
            Err(ServiceError::DependencyNotReady(api))
        );
        assert_eq!(supervisor.report_ready(api), Ok(ServiceState::Ready));
        assert_eq!(supervisor.begin_service(frontend), Ok(()));
        assert_eq!(
            supervisor.services(),
            vec![
                (database, "database".into(), true, ServiceState::Ready),
                (api, "api".into(), true, ServiceState::Ready),
                (frontend, "frontend".into(), false, ServiceState::Starting),
            ]
        );

        // Shutdown order remains based on actual successful startup order.
        assert_eq!(
            supervisor.request_shutdown(),
            Ok(vec![frontend, api, database])
        );
    }

    #[test]
    fn dependency_registration_rejects_unknown_and_duplicate_ids_atomically() {
        let supervisor = ServiceSupervisor::with_capacity(4).unwrap();
        let database = supervisor.register("database", true).unwrap();
        let unknown = ServiceId(99);

        assert_eq!(
            supervisor.register_with_dependencies("unknown", true, &[unknown]),
            Err(ServiceError::UnknownDependency(unknown))
        );
        assert_eq!(
            supervisor.register_with_dependencies("duplicate", true, &[database, database]),
            Err(ServiceError::DuplicateDependency(database))
        );
        // Failed registrations do not consume IDs or leave partial entries.
        let api = supervisor
            .register_with_dependencies("api", true, &[database])
            .unwrap();
        assert_eq!(api, ServiceId(1));
        assert_eq!(supervisor.services().len(), 2);
    }

    #[test]
    fn optional_services_do_not_gate_readiness_and_may_finish_afterward() {
        let supervisor = ServiceSupervisor::with_capacity(3).unwrap();
        let required = supervisor.register("required", true).unwrap();
        let optional = supervisor.register("optional", false).unwrap();

        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));
        supervisor.begin_service(required).unwrap();
        supervisor.begin_service(optional).unwrap();
        assert_eq!(supervisor.report_ready(required), Ok(ServiceState::Ready));
        assert_eq!(
            supervisor.service_state(optional),
            Ok(ServiceState::Starting)
        );

        // Optional startup can complete after application readiness without
        // demoting the supervisor or changing the required-service gate.
        assert_eq!(supervisor.report_ready(optional), Ok(ServiceState::Ready));
        assert_eq!(supervisor.lifecycle(), ServiceState::Ready);
    }

    #[test]
    fn optional_only_startup_is_ready_immediately_but_services_still_transition() {
        let supervisor = ServiceSupervisor::with_capacity(1).unwrap();
        let optional = supervisor.register("metrics", false).unwrap();

        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Ready));
        supervisor.begin_service(optional).unwrap();
        assert_eq!(supervisor.lifecycle(), ServiceState::Ready);
        assert_eq!(supervisor.report_ready(optional), Ok(ServiceState::Ready));
    }

    #[test]
    fn optional_service_startup_failure_after_readiness_is_a_runtime_failure() {
        let supervisor = ServiceSupervisor::with_capacity(1).unwrap();
        let optional = supervisor.register("metrics", false).unwrap();
        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Ready));
        supervisor.begin_service(optional).unwrap();

        assert_eq!(
            supervisor.fail_runtime(optional, "metrics initialization failed"),
            Ok(vec![optional])
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Draining);
        assert_eq!(
            supervisor.failures(),
            vec![ServiceFailure {
                service: optional,
                name: "metrics".into(),
                during_startup: false,
                message: "metrics initialization failed".into(),
            }]
        );
    }

    #[test]
    fn startup_failure_drains_in_reverse_start_order_and_finishes() {
        let (supervisor, first, second) = started(true);
        let third = supervisor.register("unreachable", false).unwrap();
        assert_eq!(third, ServiceId(2));
        assert_eq!(
            supervisor.fail_startup(second, "bind failed").unwrap(),
            vec![second, first]
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Draining);
        assert!(supervisor.failures()[0].during_startup);
        assert_eq!(
            supervisor.notify_stopped(second).unwrap(),
            ServiceState::Draining
        );
        assert_eq!(
            supervisor.notify_stopped(first).unwrap(),
            ServiceState::Stopped
        );
        assert_eq!(
            supervisor.notify_stopped(first).unwrap(),
            ServiceState::Stopped
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
    }

    #[test]
    fn startup_cancellation_drains_all_started_services_in_reverse_order() {
        let (supervisor, first, second) = started(true);
        assert_eq!(supervisor.cancel_startup().unwrap(), vec![second, first]);
        assert_eq!(
            supervisor.notify_stopped(second).unwrap(),
            ServiceState::Draining
        );
        assert_eq!(
            supervisor.notify_stopped(first).unwrap(),
            ServiceState::Stopped
        );
    }

    #[test]
    fn startup_failure_cleans_failed_service_and_uses_actual_start_order() {
        let supervisor = ServiceSupervisor::with_capacity(4).unwrap();
        let registered_first = supervisor.register("registered-first", true).unwrap();
        let failed = supervisor.register("failed", false).unwrap();
        let registered_last = supervisor.register("registered-last", true).unwrap();
        let never_started = supervisor.register("never-started", false).unwrap();
        supervisor.begin_startup().unwrap();

        // Start order intentionally differs from registration order. The
        // failing service acquired resources, so it must be part of cleanup.
        supervisor.begin_service(registered_last).unwrap();
        supervisor.begin_service(failed).unwrap();
        supervisor.begin_service(registered_first).unwrap();
        assert_eq!(
            supervisor.fail_startup(failed, "initialization failed"),
            Ok(vec![registered_first, failed, registered_last])
        );
        assert_eq!(supervisor.service_state(failed), Ok(ServiceState::Draining));
        assert_eq!(
            supervisor.service_state(never_started),
            Ok(ServiceState::Stopped)
        );
        assert_eq!(
            supervisor.failures(),
            vec![ServiceFailure {
                service: failed,
                name: "failed".to_owned(),
                during_startup: true,
                message: "initialization failed".to_owned(),
            }]
        );
    }

    #[test]
    fn startup_cancellation_with_no_started_services_stops_immediately() {
        let supervisor = ServiceSupervisor::with_capacity(2).unwrap();
        let required = supervisor.register("required", true).unwrap();
        let optional = supervisor.register("optional", false).unwrap();
        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));

        assert_eq!(supervisor.cancel_startup(), Ok(Vec::new()));
        assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
        assert_eq!(
            supervisor.service_state(required),
            Ok(ServiceState::Stopped)
        );
        assert_eq!(
            supervisor.service_state(optional),
            Ok(ServiceState::Stopped)
        );
    }

    #[test]
    fn runtime_failure_requests_shutdown_and_records_diagnostic() {
        let (supervisor, first, second) = started(true);
        supervisor.report_ready(first).unwrap();
        supervisor.report_ready(second).unwrap();
        assert_eq!(
            supervisor.fail_runtime(first, "lost connection").unwrap(),
            vec![second, first]
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Draining);
        assert_eq!(supervisor.failures()[0].message, "lost connection");
        assert_eq!(
            supervisor.notify_stopped(second).unwrap(),
            ServiceState::Draining
        );
        assert_eq!(
            supervisor.notify_stopped(first).unwrap(),
            ServiceState::Stopped
        );
    }

    #[test]
    fn shutdown_is_rejected_from_created_and_stopped_states() {
        let supervisor = ServiceSupervisor::new();
        assert_eq!(
            supervisor.request_shutdown(),
            Err(ServiceError::InvalidTransition {
                operation: ServiceOperation::RequestShutdown,
                state: ServiceState::Created,
            })
        );
        supervisor.begin_startup().unwrap();
        assert_eq!(supervisor.lifecycle(), ServiceState::Ready);
        assert_eq!(
            supervisor.request_shutdown().unwrap(),
            Vec::<ServiceId>::new()
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
        assert!(matches!(
            supervisor.request_shutdown(),
            Err(ServiceError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn duplicate_stop_notifications_are_idempotent_while_others_drain() {
        let (supervisor, first, second) = started(true);
        supervisor.request_shutdown().unwrap();

        assert_eq!(
            supervisor.notify_stopped(second),
            Ok(ServiceState::Draining)
        );
        assert_eq!(
            supervisor.notify_stopped(second),
            Ok(ServiceState::Draining)
        );
        assert_eq!(supervisor.service_state(first), Ok(ServiceState::Draining));
        assert_eq!(supervisor.notify_stopped(first), Ok(ServiceState::Stopped));
        assert_eq!(supervisor.notify_stopped(second), Ok(ServiceState::Stopped));
    }

    #[test]
    fn invalid_transition_edges_return_stable_errors_without_mutating_state() {
        let supervisor = ServiceSupervisor::with_capacity(2).unwrap();
        let required = supervisor.register("required", true).unwrap();
        let unknown = ServiceId(99);

        assert_eq!(
            supervisor.report_ready(required),
            Err(ServiceError::InvalidTransition {
                operation: ServiceOperation::ReportReady,
                state: ServiceState::Created,
            })
        );
        assert_eq!(
            supervisor.notify_stopped(required),
            Err(ServiceError::InvalidTransition {
                operation: ServiceOperation::NotifyStopped,
                state: ServiceState::Created,
            })
        );
        assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));
        assert_eq!(
            supervisor.begin_service(unknown),
            Err(ServiceError::UnknownService(unknown))
        );
        assert_eq!(
            supervisor.service_state(required),
            Ok(ServiceState::Created)
        );
        assert_eq!(supervisor.lifecycle(), ServiceState::Starting);
        supervisor.begin_service(required).unwrap();
        assert_eq!(supervisor.report_ready(required), Ok(ServiceState::Ready));
        assert_eq!(
            supervisor.fail_startup(required, "too late"),
            Err(ServiceError::ServiceNotStarting(required))
        );
        assert!(supervisor.failures().is_empty());
    }

    #[test]
    fn repeated_start_ready_reverse_stop_cycles_leave_no_live_service_state() {
        for cycle in 0..128 {
            let supervisor = ServiceSupervisor::with_capacity(4).unwrap();
            let ids = (0..4)
                .map(|index| {
                    supervisor
                        .register(format!("service-{index}"), true)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));
            for id in &ids {
                supervisor.begin_service(*id).unwrap();
            }
            for id in ids.iter().rev() {
                let state = supervisor.report_ready(*id).unwrap();
                assert_eq!(
                    state,
                    if *id == ids[0] {
                        ServiceState::Ready
                    } else {
                        ServiceState::Starting
                    },
                    "cycle={cycle}"
                );
            }

            assert_eq!(
                supervisor.request_shutdown().unwrap(),
                ids.iter().rev().copied().collect::<Vec<_>>(),
                "cycle={cycle}"
            );
            for (index, id) in ids.iter().rev().enumerate() {
                let state = supervisor.notify_stopped(*id).unwrap();
                assert_eq!(
                    state,
                    if index + 1 == ids.len() {
                        ServiceState::Stopped
                    } else {
                        ServiceState::Draining
                    },
                    "cycle={cycle}"
                );
            }
            assert!(
                supervisor
                    .services()
                    .iter()
                    .all(|(_, _, _, state)| *state == ServiceState::Stopped)
            );
        }
    }

    #[test]
    fn repeated_runtime_failure_drain_notify_cycles_leave_no_started_service() {
        for cycle in 0..128 {
            let supervisor = ServiceSupervisor::with_capacity(4).unwrap();
            let ids = (0..4)
                .map(|index| {
                    supervisor
                        .register(format!("service-{index}"), true)
                        .unwrap()
                })
                .collect::<Vec<_>>();

            assert_eq!(supervisor.begin_startup(), Ok(ServiceState::Starting));
            for (index, id) in ids.iter().enumerate() {
                supervisor.begin_service(*id).unwrap();
                assert_eq!(
                    supervisor.report_ready(*id).unwrap(),
                    if index + 1 == ids.len() {
                        ServiceState::Ready
                    } else {
                        ServiceState::Starting
                    },
                    "cycle={cycle}, service={id:?}"
                );
            }
            assert_eq!(supervisor.lifecycle(), ServiceState::Ready);

            let failed = ids[cycle % ids.len()];
            let message = format!("service failure in cycle {cycle}");
            assert_eq!(
                supervisor.fail_runtime(failed, message.clone()).unwrap(),
                ids.iter().rev().copied().collect::<Vec<_>>(),
                "cycle={cycle}"
            );
            assert_eq!(supervisor.lifecycle(), ServiceState::Draining);
            assert_eq!(
                supervisor.failures(),
                vec![ServiceFailure {
                    service: failed,
                    name: format!("service-{}", cycle % ids.len()),
                    during_startup: false,
                    message,
                }],
                "cycle={cycle}"
            );

            for (index, id) in ids.iter().rev().enumerate() {
                let state = supervisor.notify_stopped(*id).unwrap();
                assert_eq!(
                    state,
                    if index + 1 == ids.len() {
                        ServiceState::Stopped
                    } else {
                        ServiceState::Draining
                    },
                    "cycle={cycle}, service={id:?}"
                );
            }
            assert_eq!(supervisor.lifecycle(), ServiceState::Stopped);
            assert!(
                supervisor
                    .services()
                    .iter()
                    .all(|(_, _, _, state)| { *state == ServiceState::Stopped }),
                "cycle={cycle}"
            );
            assert!(
                ids.iter()
                    .all(|id| { supervisor.service_state(*id) == Ok(ServiceState::Stopped) }),
                "cycle={cycle}"
            );
        }
    }
}
