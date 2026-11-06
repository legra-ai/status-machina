use crate::DispatchError;

/// Compact observation for one caller-supplied event/fact step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioObservation<I: crate::MachineIds = crate::DefaultIds> {
    pub(in crate::scenario) scenario: I::Scenario,
    pub(in crate::scenario) step: usize,
    pub(in crate::scenario) input_event: I::Event,
    pub(in crate::scenario) before: I::State,
    pub(in crate::scenario) after: I::State,
    pub(in crate::scenario) kind: ScenarioStepKind<I>,
    pub(in crate::scenario) output_events: Vec<I::Event>,
    pub(in crate::scenario) effects: Vec<I::Effect>,
}

impl<I: crate::MachineIds> ScenarioObservation<I> {
    /// Return the scenario identifier.
    #[must_use]
    pub fn scenario(&self) -> &I::Scenario {
        &self.scenario
    }

    /// Return the zero-based step index.
    #[must_use]
    pub const fn step(&self) -> usize {
        self.step
    }

    /// Return the input event identifier, without its payload.
    #[must_use]
    pub fn input_event(&self) -> &I::Event {
        &self.input_event
    }

    /// Return the state before the step.
    #[must_use]
    pub fn before(&self) -> &I::State {
        &self.before
    }

    /// Return the state after the step.
    #[must_use]
    pub fn after(&self) -> &I::State {
        &self.after
    }

    /// Return the compact dispatch outcome.
    #[must_use]
    pub const fn kind(&self) -> &ScenarioStepKind<I> {
        &self.kind
    }

    /// Return produced output event identifiers, without payloads.
    #[must_use]
    pub fn output_events(&self) -> &[I::Event] {
        &self.output_events
    }

    /// Return produced effect identifiers, without payloads.
    #[must_use]
    pub fn effects(&self) -> &[I::Effect] {
        &self.effects
    }
}

/// Compact outcome recorded for a replayed event step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenarioStepKind<I: crate::MachineIds = crate::DefaultIds> {
    /// One transition was accepted.
    Transitioned(I::Transition),
    /// One declared disposition accepted the event without a state change.
    Ignored(I::Disposition),
    /// No transition or disposition accepted the event.
    Refused,
    /// Dispatch returned an error; arbitrary error text is not retained.
    DispatchFailed,
}

/// Dispatch error category without callback-provided error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioFailure {
    /// Initial instance data violated its state invariant.
    InvalidInitialData,
    /// More than one transition or disposition matched.
    Ambiguous,
    /// Candidate data violated a destination invariant.
    InvariantFailed,
    /// A transition postcondition rejected candidate data.
    PostconditionFailed,
    /// A transition action failed.
    ActionFailed,
    /// The instance revision could not advance.
    RevisionExhausted,
    /// A decision named another definition.
    DefinitionMismatch,
    /// A decision named another instance.
    InstanceMismatch,
    /// Dispatch used a stale revision.
    StaleRevision,
}

impl<I: crate::MachineIds> From<&DispatchError<I>> for ScenarioFailure {
    fn from(error: &DispatchError<I>) -> Self {
        match error {
            DispatchError::Ambiguous { .. } => Self::Ambiguous,
            DispatchError::InvariantFailed { .. } => Self::InvariantFailed,
            DispatchError::PostconditionFailed { .. } => Self::PostconditionFailed,
            DispatchError::ActionFailed { .. } => Self::ActionFailed,
            DispatchError::RevisionExhausted => Self::RevisionExhausted,
            DispatchError::StaleRevision { .. } => Self::StaleRevision,
            DispatchError::DefinitionMismatch { .. } => Self::DefinitionMismatch,
            DispatchError::InstanceMismatch { .. } => Self::InstanceMismatch,
        }
    }
}

/// Bounded replay result, explicitly limited to the supplied scenario set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScenarioResult<I: crate::MachineIds = crate::DefaultIds> {
    /// Every supplied scenario step was checked and passed the assertions.
    CompleteWithinScenarios {
        /// Number of supplied scenarios completed.
        scenarios: usize,
        /// Number of event/fact dispatch nodes visited.
        nodes: usize,
    },
    /// An independently named assertion failed on this bounded trace.
    Counterexample(CounterexampleTrace<I>),
    /// Replay stopped at a configured dynamic budget before completion.
    LimitReached {
        /// Scenario where replay stopped.
        scenario: I::Scenario,
        /// Zero-based step that could not be checked.
        step: usize,
        /// Budget that stopped replay.
        limit: ScenarioLimit,
    },
    /// Machine creation or dispatch failed during replay.
    ExecutionFailed {
        /// Scenario where execution failed.
        scenario: I::Scenario,
        /// Zero-based step; zero denotes instance creation failure.
        step: usize,
        /// Bounded failure category, without callback error text.
        failure: ScenarioFailure,
        /// Compact trace prefix including the failed step when dispatch began.
        steps: Vec<ScenarioObservation<I>>,
    },
}

/// Dynamic budget that stopped replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioLimit {
    /// Total produced output and effect intent budget.
    Outputs,
    /// Retained trace-entry budget for the current scenario.
    TraceEntries,
}

/// Result wrapper that documents bounded semantics in its type name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedScenarioReport<I: crate::MachineIds = crate::DefaultIds> {
    pub(in crate::scenario) result: ScenarioResult<I>,
}

impl<I: crate::MachineIds> BoundedScenarioReport<I> {
    /// Return the bounded replay outcome.
    #[must_use]
    pub const fn result(&self) -> &ScenarioResult<I> {
        &self.result
    }
}

/// Compact event-step prefix that falsified an independent named assertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CounterexampleTrace<I: crate::MachineIds = crate::DefaultIds> {
    pub(in crate::scenario) scenario: I::Scenario,
    pub(in crate::scenario) assertion: I::Assertion,
    pub(in crate::scenario) steps: Vec<ScenarioObservation<I>>,
}

impl<I: crate::MachineIds> CounterexampleTrace<I> {
    /// Return the scenario that produced this bounded counterexample.
    #[must_use]
    pub fn scenario(&self) -> &I::Scenario {
        &self.scenario
    }

    /// Return the independent assertion that failed.
    #[must_use]
    pub fn assertion(&self) -> &I::Assertion {
        &self.assertion
    }

    /// Return the compact trace prefix through the failing step.
    #[must_use]
    pub fn steps(&self) -> &[ScenarioObservation<I>] {
        &self.steps
    }
}
