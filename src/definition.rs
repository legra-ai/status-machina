use std::collections::HashSet;

use crate::{
    EventDisposition,
    Transition,
};

mod bindings;
mod error;
mod inspection;
mod instantiate;
mod state;
pub use error::{
    DefinitionBuildError,
    DefinitionResult,
};
pub use inspection::{
    InspectionError,
    InspectionLimits,
    InspectionLimitsBuilder,
    StateCoverage,
    StructuralCoverage,
};
pub use state::{
    StateDefinition,
    StateDefinitionBuilder,
};

pub(crate) const DEFAULT_MAX_STATES: usize = 128;
pub(crate) const DEFAULT_MAX_TRANSITIONS: usize = 512;
const DEFAULT_MAX_DISPOSITIONS: usize = 128;

/// Immutable, validated machine definition.
pub struct MachineDefinition<D, E, F, X = (), O = (), I: crate::MachineIds = crate::DefaultIds> {
    id: I::Definition,
    initial: I::State,
    states: Vec<StateDefinition<D, E, F, X, I>>,
    transitions: Vec<Transition<D, E, F, X, O, I>>,
    dispositions: Vec<EventDisposition<D, E, F, I>>,
}

impl<D, E, F, I: crate::MachineIds> MachineDefinition<D, E, F, (), (), I>
where
    D: crate::CandidateData,
{
    /// Start building a machine definition.
    #[must_use]
    pub fn builder(id: I::Definition) -> DefinitionBuilder<D, E, F, (), (), I> {
        DefinitionBuilder {
            id,
            initial: None,
            states: Vec::new(),
            transitions: Vec::new(),
            dispositions: Vec::new(),
            max_states: DEFAULT_MAX_STATES,
            max_transitions: DEFAULT_MAX_TRANSITIONS,
        }
    }
}

impl<D, E, F, X, I: crate::MachineIds> MachineDefinition<D, E, F, X, (), I>
where
    D: crate::CandidateData,
{
    /// Start building a machine definition that emits typed effect intents.
    #[must_use]
    pub fn builder_with_effects(id: I::Definition) -> DefinitionBuilder<D, E, F, X, (), I> {
        DefinitionBuilder {
            id,
            initial: None,
            states: Vec::new(),
            transitions: Vec::new(),
            dispositions: Vec::new(),
            max_states: DEFAULT_MAX_STATES,
            max_transitions: DEFAULT_MAX_TRANSITIONS,
        }
    }
}

impl<D, E, F, O, I: crate::MachineIds> MachineDefinition<D, E, F, (), O, I>
where
    D: crate::CandidateData,
{
    /// Start building a machine definition that emits typed output events.
    #[must_use]
    pub fn builder_with_output_events(id: I::Definition) -> DefinitionBuilder<D, E, F, (), O, I> {
        DefinitionBuilder {
            id,
            initial: None,
            states: Vec::new(),
            transitions: Vec::new(),
            dispositions: Vec::new(),
            max_states: DEFAULT_MAX_STATES,
            max_transitions: DEFAULT_MAX_TRANSITIONS,
        }
    }
}

impl<D, E, F, X, O, I: crate::MachineIds> MachineDefinition<D, E, F, X, O, I>
where
    D: crate::CandidateData,
{
    /// Start building a machine definition that emits typed effect and
    /// output-event intents.
    #[must_use]
    pub fn builder_with_effects_and_output_events(
        id: I::Definition,
    ) -> DefinitionBuilder<D, E, F, X, O, I> {
        DefinitionBuilder {
            id,
            initial: None,
            states: Vec::new(),
            transitions: Vec::new(),
            dispositions: Vec::new(),
            max_states: DEFAULT_MAX_STATES,
            max_transitions: DEFAULT_MAX_TRANSITIONS,
        }
    }

    /// Return the machine identifier.
    #[must_use]
    pub const fn id(&self) -> &I::Definition {
        &self.id
    }

    pub(crate) fn state(&self, id: &I::State) -> Option<&StateDefinition<D, E, F, X, I>> {
        self.states.iter().find(|state| state.id == *id)
    }

    pub(crate) fn transitions(&self) -> &[Transition<D, E, F, X, O, I>] {
        &self.transitions
    }

    pub(crate) fn dispositions(&self) -> &[EventDisposition<D, E, F, I>] {
        &self.dispositions
    }
}

/// Bounded builder for a machine definition.
pub struct DefinitionBuilder<D, E, F, X = (), O = (), I: crate::MachineIds = crate::DefaultIds> {
    id: I::Definition,
    initial: Option<I::State>,
    states: Vec<StateDefinition<D, E, F, X, I>>,
    transitions: Vec<Transition<D, E, F, X, O, I>>,
    dispositions: Vec<EventDisposition<D, E, F, I>>,
    max_states: usize,
    max_transitions: usize,
}

impl<D, E, F, X, O, I: crate::MachineIds> DefinitionBuilder<D, E, F, X, O, I> {
    /// Set the initial state.
    #[must_use]
    pub fn initial(mut self, state: I::State) -> Self {
        self.initial = Some(state);
        self
    }

    /// Set explicit graph-size limits before adding states and transitions.
    ///
    /// # Errors
    ///
    /// Returns an error for zero limits or limits below the current graph size.
    pub fn limits(
        mut self,
        max_states: usize,
        max_transitions: usize,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if max_states == 0 || max_transitions == 0 {
            return Err(DefinitionBuildError::EmptyLimits);
        }
        if self.states.len() > max_states || self.transitions.len() > max_transitions {
            return Err(DefinitionBuildError::LimitsBelowCurrentSize);
        }
        self.max_states = max_states;
        self.max_transitions = max_transitions;
        Ok(self)
    }

    /// Add a state without exceeding the configured state bound.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured state limit has been reached.
    pub fn state(
        mut self,
        state: StateDefinition<D, E, F, X, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.states.len() == self.max_states {
            return Err(DefinitionBuildError::StateLimitExceeded {
                limit: self.max_states,
            });
        }
        self.states.push(state);
        Ok(self)
    }

    /// Add a transition without exceeding the configured edge bound.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured transition limit has been reached.
    pub fn transition(
        mut self,
        transition: Transition<D, E, F, X, O, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.transitions.len() == self.max_transitions {
            return Err(DefinitionBuildError::TransitionLimitExceeded {
                limit: self.max_transitions,
            });
        }
        self.transitions.push(transition);
        Ok(self)
    }

    /// Add an explicit event-disposition rule.
    ///
    /// # Errors
    ///
    /// Returns an error when the disposition bound has been reached.
    pub fn disposition(
        mut self,
        disposition: EventDisposition<D, E, F, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.dispositions.len() == DEFAULT_MAX_DISPOSITIONS {
            return Err(DefinitionBuildError::DispositionLimitExceeded {
                limit: DEFAULT_MAX_DISPOSITIONS,
            });
        }
        self.dispositions.push(disposition);
        Ok(self)
    }

    /// Validate graph references, uniqueness, and reachability.
    ///
    /// # Errors
    ///
    /// Returns an error for missing or duplicate states/edges, missing
    /// endpoints, or states unreachable from the initial state.
    pub fn build(self) -> DefinitionResult<MachineDefinition<D, E, F, X, O, I>, I> {
        let initial = self
            .initial
            .clone()
            .ok_or(DefinitionBuildError::MissingInitialState)?;
        let state_ids = self.collect_state_ids()?;
        if !state_ids.contains(&initial) {
            return Err(DefinitionBuildError::UnknownInitialState { state: initial });
        }
        self.validate_transitions(&state_ids)?;
        self.validate_dispositions(&state_ids)?;
        self.validate_reachability(&initial, &state_ids)?;
        Ok(MachineDefinition {
            id: self.id,
            initial,
            states: self.states,
            transitions: self.transitions,
            dispositions: self.dispositions,
        })
    }

    fn collect_state_ids(&self) -> Result<HashSet<I::State>, DefinitionBuildError<I>> {
        let mut ids = HashSet::with_capacity(self.states.len());
        for state in &self.states {
            if !ids.insert(state.id.clone()) {
                return Err(DefinitionBuildError::DuplicateState {
                    state: state.id.clone(),
                });
            }
        }
        Ok(ids)
    }

    fn validate_transitions(
        &self,
        states: &HashSet<I::State>,
    ) -> Result<(), DefinitionBuildError<I>> {
        let mut transition_ids = HashSet::with_capacity(self.transitions.len());
        for transition in &self.transitions {
            if !transition_ids.insert(transition.id.clone()) {
                return Err(DefinitionBuildError::DuplicateTransition {
                    transition: transition.id.clone(),
                });
            }
            if !states.contains(&transition.from) {
                return Err(DefinitionBuildError::UnknownTransitionState {
                    transition: transition.id.clone(),
                    state: transition.from.clone(),
                });
            }
            if !states.contains(&transition.to) {
                return Err(DefinitionBuildError::UnknownTransitionState {
                    transition: transition.id.clone(),
                    state: transition.to.clone(),
                });
            }
            if self
                .states
                .iter()
                .find(|state| state.id == transition.from)
                .is_some_and(StateDefinition::terminal)
            {
                return Err(DefinitionBuildError::TerminalHasOutgoingTransition {
                    state: transition.from.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_dispositions(
        &self,
        states: &HashSet<I::State>,
    ) -> Result<(), DefinitionBuildError<I>> {
        let mut disposition_ids = HashSet::with_capacity(self.dispositions.len());
        for disposition in &self.dispositions {
            if !disposition_ids.insert(disposition.id.clone()) {
                return Err(DefinitionBuildError::DuplicateDisposition {
                    disposition: disposition.id.clone(),
                });
            }
            if !states.contains(&disposition.from) {
                return Err(DefinitionBuildError::UnknownDispositionState {
                    disposition: disposition.id.clone(),
                    state: disposition.from.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_reachability(
        &self,
        initial: &I::State,
        states: &HashSet<I::State>,
    ) -> Result<(), DefinitionBuildError<I>> {
        let mut reached = HashSet::from([initial.clone()]);
        loop {
            let previous_count = reached.len();
            for transition in &self.transitions {
                if reached.contains(&transition.from) {
                    reached.insert(transition.to.clone());
                }
            }
            if reached.len() == previous_count {
                break;
            }
        }
        if let Some(unreachable) = states.iter().find(|state| !reached.contains(*state)) {
            return Err(DefinitionBuildError::UnreachableState {
                state: unreachable.clone(),
            });
        }
        Ok(())
    }
}
