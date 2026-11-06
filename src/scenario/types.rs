use crate::Event;

const DEFAULT_SCENARIOS: usize = 64;
const DEFAULT_STEPS_PER_SCENARIO: usize = 256;
const DEFAULT_NODES: usize = 4096;
const DEFAULT_OUTPUTS: usize = 4096;
const DEFAULT_TRACE_ENTRIES: usize = 4096;
const HARD_MAX_SCENARIOS: usize = 1024;
const HARD_MAX_STEPS_PER_SCENARIO: usize = 4096;
const HARD_MAX_NODES: usize = 65_536;
const HARD_MAX_OUTPUTS: usize = 65_536;
const HARD_MAX_TRACE_ENTRIES: usize = 65_536;

#[path = "assertion.rs"]
pub(super) mod assertion;
#[path = "trace.rs"]
mod trace;

pub use assertion::{
    ScenarioAssertion,
    ScenarioAssertionBuildError,
};
pub use trace::{
    BoundedScenarioReport,
    CounterexampleTrace,
    ScenarioFailure,
    ScenarioLimit,
    ScenarioObservation,
    ScenarioResult,
    ScenarioStepKind,
};

/// Positive resource limits for finite scenario replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioLimits {
    pub(super) max_scenarios: usize,
    pub(super) max_steps_per_scenario: usize,
    pub(super) max_nodes: usize,
    pub(super) max_outputs: usize,
    pub(super) max_trace_entries: usize,
}

impl ScenarioLimits {
    /// Start bounded replay limits with conservative defaults.
    #[must_use]
    pub const fn builder() -> ScenarioLimitsBuilder {
        ScenarioLimitsBuilder {
            max_scenarios: DEFAULT_SCENARIOS,
            max_steps_per_scenario: DEFAULT_STEPS_PER_SCENARIO,
            max_nodes: DEFAULT_NODES,
            max_outputs: DEFAULT_OUTPUTS,
            max_trace_entries: DEFAULT_TRACE_ENTRIES,
        }
    }
}

/// Builder for finite scenario replay limits.
pub struct ScenarioLimitsBuilder {
    max_scenarios: usize,
    max_steps_per_scenario: usize,
    max_nodes: usize,
    max_outputs: usize,
    max_trace_entries: usize,
}

impl ScenarioLimitsBuilder {
    /// Bound the number of supplied scenarios.
    #[must_use]
    pub const fn max_scenarios(mut self, limit: usize) -> Self {
        self.max_scenarios = limit;
        self
    }

    /// Bound ordered event/fact steps in each scenario.
    #[must_use]
    pub const fn max_steps_per_scenario(mut self, limit: usize) -> Self {
        self.max_steps_per_scenario = limit;
        self
    }

    /// Bound total dispatch nodes across the supplied scenario set.
    #[must_use]
    pub const fn max_nodes(mut self, limit: usize) -> Self {
        self.max_nodes = limit;
        self
    }

    /// Bound combined output and effect intents produced across the scenarios.
    #[must_use]
    pub const fn max_outputs(mut self, limit: usize) -> Self {
        self.max_outputs = limit;
        self
    }

    /// Bound retained compact trace entries for the current scenario.
    #[must_use]
    pub const fn max_trace_entries(mut self, limit: usize) -> Self {
        self.max_trace_entries = limit;
        self
    }

    /// Validate and build limits within hard implementation ceilings.
    ///
    /// # Errors
    ///
    /// Returns an error for zero limits or limits above a hard ceiling.
    pub const fn build(self) -> Result<ScenarioLimits, ScenarioLimitsBuildError> {
        if self.max_scenarios == 0
            || self.max_scenarios > HARD_MAX_SCENARIOS
            || self.max_steps_per_scenario == 0
            || self.max_steps_per_scenario > HARD_MAX_STEPS_PER_SCENARIO
            || self.max_nodes == 0
            || self.max_nodes > HARD_MAX_NODES
            || self.max_outputs == 0
            || self.max_outputs > HARD_MAX_OUTPUTS
            || self.max_trace_entries == 0
            || self.max_trace_entries > HARD_MAX_TRACE_ENTRIES
        {
            return Err(ScenarioLimitsBuildError::OutsideSupportedRange);
        }
        Ok(ScenarioLimits {
            max_scenarios: self.max_scenarios,
            max_steps_per_scenario: self.max_steps_per_scenario,
            max_nodes: self.max_nodes,
            max_outputs: self.max_outputs,
            max_trace_entries: self.max_trace_entries,
        })
    }
}

/// Scenario replay limits were outside supported positive ranges.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScenarioLimitsBuildError {
    /// A limit was zero or exceeded its hard ceiling.
    #[error("scenario replay limit is outside the supported range")]
    OutsideSupportedRange,
}

/// A bounded finite set of ordered caller-supplied event/fact scenarios.
pub struct ScenarioSet<E, F, I: crate::MachineIds = crate::DefaultIds> {
    pub(super) limits: ScenarioLimits,
    pub(super) scenarios: Vec<FiniteScenario<E, F, I>>,
}

pub(super) struct FiniteScenario<E, F, I: crate::MachineIds = crate::DefaultIds> {
    pub(super) id: I::Scenario,
    pub(super) steps: Vec<(Event<E, I>, F)>,
}

impl<E, F, I: crate::MachineIds> ScenarioSet<E, F, I> {
    /// Start a scenario set using explicit replay limits.
    #[must_use]
    pub fn builder(limits: ScenarioLimits) -> ScenarioSetBuilder<E, F, I> {
        ScenarioSetBuilder {
            limits,
            scenarios: Vec::new(),
            total_steps: 0,
        }
    }
}

/// Builder that bounds scenarios and their accumulated step vectors.
pub struct ScenarioSetBuilder<E, F, I: crate::MachineIds = crate::DefaultIds> {
    limits: ScenarioLimits,
    // bounded: insertion checks both scenario count and per-scenario steps.
    scenarios: Vec<FiniteScenario<E, F, I>>,
    total_steps: usize,
}

impl<E, F, I: crate::MachineIds> ScenarioSetBuilder<E, F, I> {
    /// Add one ordered scenario, stopping before accumulating beyond its step
    /// cap.
    ///
    /// # Errors
    ///
    /// Returns an error if the scenario count or step count exceeds its limit.
    pub fn scenario(
        mut self,
        id: I::Scenario,
        steps: impl IntoIterator<Item = (Event<E, I>, F)>,
    ) -> Result<Self, ScenarioSetBuildError> {
        if self.scenarios.len() == self.limits.max_scenarios {
            return Err(ScenarioSetBuildError::ScenarioLimitExceeded {
                limit: self.limits.max_scenarios,
            });
        }
        let mut bounded_steps = Vec::new();
        for step in steps {
            if bounded_steps.len() == self.limits.max_steps_per_scenario {
                return Err(ScenarioSetBuildError::StepLimitExceeded {
                    limit: self.limits.max_steps_per_scenario,
                });
            }
            if self.total_steps == self.limits.max_nodes {
                return Err(ScenarioSetBuildError::TotalNodeLimitExceeded {
                    limit: self.limits.max_nodes,
                });
            }
            bounded_steps.push(step);
            self.total_steps += 1;
        }
        self.scenarios.push(FiniteScenario {
            id,
            steps: bounded_steps,
        });
        Ok(self)
    }

    /// Build a non-empty finite scenario set.
    ///
    /// # Errors
    ///
    /// Returns an error when no scenarios were supplied.
    pub fn build(self) -> Result<ScenarioSet<E, F, I>, ScenarioSetBuildError> {
        if self.scenarios.is_empty() {
            return Err(ScenarioSetBuildError::Empty);
        }
        Ok(ScenarioSet {
            limits: self.limits,
            scenarios: self.scenarios,
        })
    }
}

/// Scenario set construction exceeded its caller-supplied bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScenarioSetBuildError {
    /// No scenario was supplied.
    #[error("scenario set is empty")]
    Empty,
    /// Supplied scenario count exceeded its bound.
    #[error("scenario count limit {limit} exceeded")]
    ScenarioLimitExceeded {
        /// Configured maximum scenario count.
        limit: usize,
    },
    /// Supplied ordered step count exceeded its bound.
    #[error("scenario step limit {limit} exceeded")]
    StepLimitExceeded {
        /// Configured maximum steps per scenario.
        limit: usize,
    },
    /// Aggregate supplied step count exceeded the total node budget.
    #[error("total scenario node limit {limit} exceeded")]
    TotalNodeLimitExceeded {
        /// Configured maximum total node count.
        limit: usize,
    },
}
