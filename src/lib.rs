//! Caller-driven extended finite-state machine kernel.
//!
//! Definitions are immutable. An instance changes only when its owner calls
//! [`MachineInstance::dispatch`]; the crate creates no tasks, threads, timers,
//! clocks, I/O, or persistence.
//!
//! Machine data implements [`CandidateData`] to provide isolated snapshots;
//! callbacks must obey its purity and ownership contract.

mod action;
mod behavior;
mod candidate_data;
mod capability;
mod composition;
mod condition;
mod decision;
mod definition;
mod diagram_labels;
mod disposition;
mod identifier;
mod identity_map;
mod instance;
mod machine_ids;
mod output_event;
mod record;
mod retry;
mod scenario;
mod transition;

pub use action::{
    ActionBuildError,
    LocalAction,
    LocalActionBuilder,
};
pub use behavior::{
    BehaviorId,
    BehaviorRef,
    BehaviorRegistry,
    BehaviorRegistryBuilder,
    BehaviorRole,
    BindingError,
    BoundInvariant,
    ImplementationId,
};
pub use candidate_data::CandidateData;
pub use capability::{
    Capability,
    CapabilityBuildError,
    CapabilityBuilder,
    CapabilityReport,
    CapabilityStatus,
};
pub use composition::{
    CapabilitySnapshot,
    CapabilitySnapshotBuildError,
    CapabilitySnapshotBuilder,
    Composition,
    CompositionBuildError,
    CompositionBuilder,
    InstanceAddress,
    InstanceRegistry,
    InstanceRegistryBuildError,
    InstanceRegistryBuilder,
    Route,
    RouteBatchError,
    RouteBatchErrorKind,
    RouteBuildError,
    RouteBuilder,
    RoutedBatch,
    RoutedItem,
};
pub use condition::{
    Condition,
    ConditionBuildError,
    ConditionBuilder,
    ConditionGroupBuilder,
    ConditionKind,
    ConditionNotBuilder,
};
pub use decision::{
    MachineDecision,
    ValidatedDecision,
};
pub use definition::{
    DefinitionBuildError,
    DefinitionBuilder,
    DefinitionResult,
    InspectionError,
    InspectionLimits,
    InspectionLimitsBuilder,
    MachineDefinition,
    StateCoverage,
    StateDefinition,
    StateDefinitionBuilder,
    StructuralCoverage,
};
pub use diagram_labels::DiagramLabels;
pub use disposition::{
    DispositionBuildError,
    EventDisposition,
    EventDispositionBuilder,
};
pub use identifier::{
    ActionId,
    AssertionId,
    CapabilityId,
    ConditionId,
    DispositionId,
    EffectId,
    EventId,
    IdentifierError,
    MachineId,
    MachineInstanceId,
    OperationId,
    RoutingScope,
    ScenarioId,
    StateId,
    TransitionId,
};
pub use identity_map::{
    IdentityMap,
    IdentityMapError,
};
pub(crate) use instance::MachineInstanceDefinition;
pub use instance::{
    DispatchError,
    DispatchOutcome,
    EffectIntent,
    MachineInstance,
    TransitionRecord,
};
pub use machine_ids::{
    DefaultIds,
    Identity,
    MachineIds,
};
pub use output_event::{
    OutputEvent,
    OutputEventBuildError,
    OutputEventBuilder,
    OutputEventIntent,
};
pub use record::{
    BehaviorUse,
    ConditionRecord,
    DefinitionRecord,
    DispositionRecord,
    RecordBuildError,
    RecordBuildResult,
    StateRecord,
    TransitionDefinitionRecord,
};
pub use retry::{
    RetryPolicy,
    RetryPolicyBuildError,
    RetryPolicyBuilder,
    RetrySchedule,
    RetryWaitError,
    RetryWaits,
    RetryWaitsBuilder,
    WaitCorrelation,
    WakeIntent,
};
pub use scenario::{
    BoundedScenarioReport,
    CounterexampleTrace,
    ScenarioAssertion,
    ScenarioAssertionBuildError,
    ScenarioFailure,
    ScenarioLimit,
    ScenarioLimits,
    ScenarioLimitsBuildError,
    ScenarioLimitsBuilder,
    ScenarioObservation,
    ScenarioReplay,
    ScenarioReplayBuildError,
    ScenarioResult,
    ScenarioSet,
    ScenarioSetBuildError,
    ScenarioSetBuilder,
    ScenarioStepKind,
};
pub use transition::{
    Effect,
    EffectBuildError,
    EffectBuilder,
    Event,
    Transition,
    TransitionBuildError,
    TransitionBuilder,
    TransitionKind,
};
