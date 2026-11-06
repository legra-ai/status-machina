//! Bounded replay of caller-supplied finite event/fact scenarios.

mod replay;
mod types;

pub use replay::{
    ScenarioReplay,
    ScenarioReplayBuildError,
};
pub use types::{
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
    ScenarioResult,
    ScenarioSet,
    ScenarioSetBuildError,
    ScenarioSetBuilder,
    ScenarioStepKind,
};
