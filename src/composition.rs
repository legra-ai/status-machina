mod registry;
mod route;
mod router;
mod snapshot;

pub use registry::{
    InstanceAddress,
    InstanceRegistry,
    InstanceRegistryBuildError,
    InstanceRegistryBuilder,
};
pub use route::{
    Route,
    RouteBuildError,
    RouteBuilder,
};
pub use router::{
    Composition,
    CompositionBuildError,
    CompositionBuilder,
    RouteBatchError,
    RouteBatchErrorKind,
    RoutedBatch,
    RoutedItem,
};
pub use snapshot::{
    CapabilitySnapshot,
    CapabilitySnapshotBuildError,
    CapabilitySnapshotBuilder,
};

pub(super) const DEFAULT_MAX_INSTANCES: usize = 128;
pub(super) const DEFAULT_MAX_ROUTES: usize = 128;
pub(super) const DEFAULT_MAX_BATCH: usize = 128;
pub(super) const DEFAULT_MAX_CAPABILITY_OBSERVATIONS: usize = 256;
pub(super) const HARD_MAX_INSTANCES: usize = 512;
pub(super) const HARD_MAX_ROUTES: usize = 512;
pub(super) const HARD_MAX_BATCH: usize = 256;
pub(super) const HARD_MAX_CAPABILITY_OBSERVATIONS: usize = 1024;
pub(super) const HARD_MAX_INSTANCE_EVENT_IDS: usize = 1024;
pub(super) const HARD_MAX_REGISTRY_EVENT_ENTRIES: usize = 65_536;
