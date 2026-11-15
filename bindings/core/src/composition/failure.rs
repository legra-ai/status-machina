//! Typed composition rejections crossing to the native exception translators.
use serde::Serialize;
use status_machina::{
    CapabilitySnapshotBuildError,
    CompositionBindingError,
    CompositionBuildError,
    InstanceRegistryBuildError,
    PropagationLimitsError,
    RouteBatchErrorKind,
    RouteBuildError,
};

use crate::error::Error;
use crate::value::Host;

/// A kernel composition rejection with its native lookup context.
///
/// `family` names the kernel error type and `reason` its variant. `entry` is
/// the zero-based position in the list that `registry` or `list` names,
/// `route` the position of a route and `address` an address index of the call.
#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub family: &'static str,
    pub reason: &'static str,
    pub message: String,
    pub registry: Option<&'static str>,
    pub list: Option<&'static str>,
    pub entry: Option<usize>,
    pub route: Option<usize>,
    pub address: Option<u32>,
    pub limit: Option<usize>,
}
impl Failure {
    fn new(family: &'static str, reason: &'static str, message: impl ToString) -> Self {
        Self {
            code: "composition",
            family,
            reason,
            message: message.to_string(),
            registry: None,
            list: None,
            entry: None,
            route: None,
            address: None,
            limit: None,
        }
    }
    pub(crate) fn registry(
        registry: &'static str,
        entry: Option<usize>,
        error: InstanceRegistryBuildError,
    ) -> Self {
        let (reason, limit) = match error {
            InstanceRegistryBuildError::InvalidLimit => ("invalid_limit", None),
            InstanceRegistryBuildError::LimitBelowCurrentSize => ("limit_below_current_size", None),
            InstanceRegistryBuildError::AddressMismatch => ("address_mismatch", None),
            InstanceRegistryBuildError::DuplicateInstance => ("duplicate_instance", None),
            InstanceRegistryBuildError::LimitExceeded { limit } => ("limit_exceeded", Some(limit)),
            InstanceRegistryBuildError::InstanceEventLimitExceeded { limit } => {
                ("instance_event_limit_exceeded", Some(limit))
            }
            InstanceRegistryBuildError::AggregateEventLimitExceeded { limit } => {
                ("aggregate_event_limit_exceeded", Some(limit))
            }
        };
        let mut failure = Self::new("registry", reason, error);
        failure.registry = Some(registry);
        failure.entry = entry;
        failure.limit = limit;
        failure
    }
    pub(crate) fn definition(route: Option<usize>, error: CompositionBuildError) -> Self {
        let (reason, limit) = match error {
            CompositionBuildError::InvalidLimits => ("invalid_limits", None),
            CompositionBuildError::LimitBelowCurrentSize => ("limit_below_current_size", None),
            CompositionBuildError::DuplicateRoute => ("duplicate_route", None),
            CompositionBuildError::RouteLimitExceeded { limit } => {
                ("route_limit_exceeded", Some(limit))
            }
            CompositionBuildError::UnregisteredSource => ("unregistered_source", None),
            CompositionBuildError::UnregisteredDestination => ("unregistered_destination", None),
            CompositionBuildError::SourcePayloadMismatch => ("source_payload_mismatch", None),
            CompositionBuildError::DestinationPayloadMismatch => {
                ("destination_payload_mismatch", None)
            }
            CompositionBuildError::UndeclaredSourceEvent => ("undeclared_source_event", None),
            CompositionBuildError::UndeclaredDestinationEvent => {
                ("undeclared_destination_event", None)
            }
        };
        let mut failure = Self::new("definition", reason, error);
        failure.route = route;
        failure.limit = limit;
        failure
    }
    pub(crate) fn route(route: usize, error: RouteBuildError) -> Self {
        let reason = match error {
            RouteBuildError::MissingDestination => "missing_destination",
            RouteBuildError::MissingInputEvent => "missing_input_event",
        };
        let mut failure = Self::new("route", reason, error);
        failure.route = Some(route);
        failure
    }
    pub(crate) fn batch(error: RouteBatchErrorKind) -> Self {
        let (reason, limit) = match error {
            RouteBatchErrorKind::UnregisteredSource => ("unregistered_source", None),
            RouteBatchErrorKind::BatchLimitExceeded { limit } => {
                ("batch_limit_exceeded", Some(limit))
            }
        };
        let mut failure = Self::new("batch", reason, error);
        failure.limit = limit;
        failure
    }
    pub(crate) fn binding(entry: Option<usize>, error: CompositionBindingError) -> Self {
        let reason = match error {
            CompositionBindingError::DuplicateInstance => "duplicate_instance",
            CompositionBindingError::UnregisteredInstance => "unregistered_instance",
            CompositionBindingError::IdentityOrInventoryMismatch => {
                "identity_or_inventory_mismatch"
            }
            CompositionBindingError::SignatureMismatch => "signature_mismatch",
            CompositionBindingError::MissingInstance => "missing_instance",
        };
        let mut failure = Self::new("binding", reason, error);
        failure.entry = entry;
        failure
    }
    /// Rust's exclusive borrows make binding one instance at two addresses
    /// unrepresentable; native callers report it before any lock is taken.
    #[must_use]
    pub fn aliased(entry: usize) -> Self {
        let mut failure = Self::new(
            "binding",
            "aliased_instance",
            "one machine instance cannot be bound exclusively at two addresses",
        );
        failure.entry = Some(entry);
        failure
    }
    pub(crate) fn limits(error: PropagationLimitsError) -> Self {
        Self::new("limits", "invalid_limits", error)
    }
    pub(crate) fn snapshot(
        list: Option<&'static str>,
        entry: Option<usize>,
        error: CapabilitySnapshotBuildError,
    ) -> Self {
        let (reason, limit) = match error {
            CapabilitySnapshotBuildError::InvalidLimit => ("invalid_limit", None),
            CapabilitySnapshotBuildError::LimitBelowCurrentSize => {
                ("limit_below_current_size", None)
            }
            CapabilitySnapshotBuildError::InstanceMismatch => ("instance_mismatch", None),
            CapabilitySnapshotBuildError::DuplicateRevision => ("duplicate_revision", None),
            CapabilitySnapshotBuildError::DuplicateCapability => ("duplicate_capability", None),
            CapabilitySnapshotBuildError::LimitExceeded { limit } => {
                ("limit_exceeded", Some(limit))
            }
        };
        let mut failure = Self::new("snapshot", reason, error);
        failure.list = list;
        failure.entry = entry;
        failure.limit = limit;
        failure
    }
    pub(crate) fn foreign_snapshot() -> Self {
        Self::new(
            "snapshot",
            "foreign_snapshot",
            "capability snapshot belongs to another composition",
        )
    }
    pub(crate) fn with_address(mut self, address: u32) -> Self {
        self.address = Some(address);
        self
    }
    /// Serialize for a native exception translator.
    #[must_use]
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("typed composition failure metadata is serializable")
    }
}

/// Either a kernel composition rejection or an ordinary bridge error, such as
/// a disposed instance, a busy resource or an exhausted value arena.
#[derive(Debug)]
pub enum Fault<H: Host> {
    Composition(Box<Failure>),
    Core(Error<H>),
}
impl<H: Host> From<Failure> for Fault<H> {
    fn from(failure: Failure) -> Self {
        Self::Composition(Box::new(failure))
    }
}
impl<H: Host> From<Box<Failure>> for Fault<H> {
    fn from(failure: Box<Failure>) -> Self {
        Self::Composition(failure)
    }
}
impl<H: Host> From<Error<H>> for Fault<H> {
    fn from(error: Error<H>) -> Self {
        Self::Core(error)
    }
}
