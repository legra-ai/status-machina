mod effect;
pub(crate) use effect::EffectConstructor;
pub use effect::{
    Effect,
    EffectBuildError,
    EffectBuilder,
};

use crate::{
    Condition,
    LocalAction,
    OutputEvent,
};

/// A caller-provided event and its typed payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event<E, I: crate::MachineIds = crate::DefaultIds> {
    id: I::Event,
    payload: E,
}

impl<E, I: crate::MachineIds> Event<E, I> {
    /// Construct an event with an explicit identifier and payload.
    #[must_use]
    pub const fn new(id: I::Event, payload: E) -> Self {
        Self { id, payload }
    }

    /// Return the event identifier.
    #[must_use]
    pub const fn id(&self) -> &I::Event {
        &self.id
    }

    /// Return the typed event payload.
    #[must_use]
    pub const fn payload(&self) -> &E {
        &self.payload
    }
}

/// One validated transition definition.
pub struct Transition<D, E, F, X = (), O = (), I: crate::MachineIds = crate::DefaultIds> {
    pub(crate) id: I::Transition,
    pub(crate) from: I::State,
    pub(crate) event: I::Event,
    pub(crate) to: I::State,
    pub(crate) guard: Option<Condition<D, E, F, I>>,
    pub(crate) postconditions: Vec<Condition<D, E, F, I>>,
    pub(crate) actions: Vec<LocalAction<D, E, F, I>>,
    pub(crate) effects: Vec<Effect<D, E, F, X, I>>,
    pub(crate) output_events: Vec<OutputEvent<D, E, F, O, I>>,
    pub(crate) kind: TransitionKind,
}

impl<D, E, F, X, O, I: crate::MachineIds> Transition<D, E, F, X, O, I> {
    pub(crate) fn bindings(&self) -> impl Iterator<Item = &crate::BehaviorRef<I>> {
        self.guard
            .iter()
            .flat_map(Condition::binding_refs)
            .chain(self.postconditions.iter().flat_map(Condition::binding_refs))
            .chain(self.actions.iter().filter_map(LocalAction::binding))
            .chain(self.effects.iter().filter_map(Effect::binding))
            .chain(self.output_events.iter().filter_map(OutputEvent::binding))
    }
}

impl<D, E, F, I: crate::MachineIds> Transition<D, E, F, (), (), I> {
    /// Start building a transition.
    #[must_use]
    pub fn builder(id: I::Transition) -> TransitionBuilder<D, E, F, (), (), I> {
        TransitionBuilder {
            id,
            from: None,
            event: None,
            to: None,
            guard: None,
            postconditions: Vec::new(),
            actions: Vec::new(),
            effects: Vec::new(),
            output_events: Vec::new(),
            kind: TransitionKind::External,
        }
    }
}

impl<D, E, F, X, I: crate::MachineIds> Transition<D, E, F, X, (), I> {
    /// Start building a transition that emits typed effect intents.
    #[must_use]
    pub fn builder_with_effects(id: I::Transition) -> TransitionBuilder<D, E, F, X, (), I> {
        TransitionBuilder {
            id,
            from: None,
            event: None,
            to: None,
            guard: None,
            postconditions: Vec::new(),
            actions: Vec::new(),
            effects: Vec::new(),
            output_events: Vec::new(),
            kind: TransitionKind::External,
        }
    }
}

impl<D, E, F, O, I: crate::MachineIds> Transition<D, E, F, (), O, I> {
    /// Start building a transition that emits typed output-event intents.
    #[must_use]
    pub fn builder_with_output_events(id: I::Transition) -> TransitionBuilder<D, E, F, (), O, I> {
        TransitionBuilder {
            id,
            from: None,
            event: None,
            to: None,
            guard: None,
            postconditions: Vec::new(),
            actions: Vec::new(),
            effects: Vec::new(),
            output_events: Vec::new(),
            kind: TransitionKind::External,
        }
    }
}

impl<D, E, F, X, O, I: crate::MachineIds> Transition<D, E, F, X, O, I> {
    /// Start building a transition that emits both effect and output-event
    /// intents.
    #[must_use]
    pub fn builder_with_effects_and_output_events(
        id: I::Transition,
    ) -> TransitionBuilder<D, E, F, X, O, I> {
        TransitionBuilder {
            id,
            from: None,
            event: None,
            to: None,
            guard: None,
            postconditions: Vec::new(),
            actions: Vec::new(),
            effects: Vec::new(),
            output_events: Vec::new(),
            kind: TransitionKind::External,
        }
    }

    /// Return the transition identifier.
    #[must_use]
    pub const fn id(&self) -> &I::Transition {
        &self.id
    }
}

/// Builder for one transition edge.
pub struct TransitionBuilder<D, E, F, X = (), O = (), I: crate::MachineIds = crate::DefaultIds> {
    id: I::Transition,
    from: Option<I::State>,
    event: Option<I::Event>,
    to: Option<I::State>,
    guard: Option<Condition<D, E, F, I>>,
    postconditions: Vec<Condition<D, E, F, I>>,
    actions: Vec<LocalAction<D, E, F, I>>,
    effects: Vec<Effect<D, E, F, X, I>>,
    output_events: Vec<OutputEvent<D, E, F, O, I>>,
    kind: TransitionKind,
}

impl<D, E, F, X, O, I: crate::MachineIds> TransitionBuilder<D, E, F, X, O, I> {
    /// Set the source state.
    #[must_use]
    pub fn from(mut self, state: I::State) -> Self {
        self.from = Some(state);
        self
    }

    /// Set the event that selects this edge.
    #[must_use]
    pub fn on(mut self, event: I::Event) -> Self {
        self.event = Some(event);
        self
    }

    /// Set the destination state.
    #[must_use]
    pub fn to(mut self, state: I::State) -> Self {
        self.to = Some(state);
        self
    }

    /// Set the named pure guard evaluated against unchanged pre-transition
    /// data.
    #[must_use]
    pub fn when(mut self, guard: Condition<D, E, F, I>) -> Self {
        self.guard = Some(guard);
        self
    }

    /// Add one condition checked against candidate data before acceptance.
    ///
    /// # Errors
    ///
    /// Returns an error when the transition already has 16 postconditions.
    pub fn postcondition(
        mut self,
        condition: Condition<D, E, F, I>,
    ) -> Result<Self, TransitionBuildError> {
        if self.postconditions.len() == 16 {
            return Err(TransitionBuildError::PostconditionLimitExceeded);
        }
        self.postconditions.push(condition);
        Ok(self)
    }

    /// Add one candidate-data action in declared execution order.
    ///
    /// # Errors
    ///
    /// Returns an error when the transition already has 16 actions.
    pub fn action(mut self, action: LocalAction<D, E, F, I>) -> Result<Self, TransitionBuildError> {
        if self.actions.len() == 16 {
            return Err(TransitionBuildError::ActionLimitExceeded);
        }
        self.actions.push(action);
        Ok(self)
    }

    /// Make a self-transition internal: only transition actions will run.
    #[must_use]
    pub fn internal(mut self) -> Self {
        self.kind = TransitionKind::Internal;
        self
    }

    /// Add one typed effect intent, with a fixed per-transition bound of 16.
    ///
    /// # Errors
    ///
    /// Returns an error when 16 effect intents have already been added.
    pub fn effect(mut self, effect: Effect<D, E, F, X, I>) -> Result<Self, TransitionBuildError> {
        if self.effects.len() == 16 {
            return Err(TransitionBuildError::EffectLimitExceeded);
        }
        self.effects.push(effect);
        Ok(self)
    }

    /// Add one typed caller-owned output event, with a fixed per-transition
    /// bound of 16.
    ///
    /// The payload is constructed only after candidate checks succeed. It is
    /// returned as data; the status-machina never dispatches it or invokes a
    /// handler.
    ///
    /// # Errors
    ///
    /// Returns an error when 16 output events have already been added.
    pub fn output_event(
        mut self,
        output_event: OutputEvent<D, E, F, O, I>,
    ) -> Result<Self, TransitionBuildError> {
        if self.output_events.len() == 16 {
            return Err(TransitionBuildError::OutputEventLimitExceeded);
        }
        self.output_events.push(output_event);
        Ok(self)
    }

    /// Validate and build the edge after its endpoints and event are supplied.
    ///
    /// # Errors
    ///
    /// Returns an error when the source, event, or destination was not
    /// supplied.
    pub fn build(self) -> Result<Transition<D, E, F, X, O, I>, TransitionBuildError> {
        let from = self.from.ok_or(TransitionBuildError::MissingSource)?;
        let event = self.event.ok_or(TransitionBuildError::MissingEvent)?;
        let to = self.to.ok_or(TransitionBuildError::MissingDestination)?;
        if self.kind == TransitionKind::Internal && from != to {
            return Err(TransitionBuildError::InternalTransitionMustBeSelf);
        }
        Ok(Transition {
            id: self.id,
            from,
            event,
            to,
            guard: self.guard,
            postconditions: self.postconditions,
            actions: self.actions,
            effects: self.effects,
            output_events: self.output_events,
            kind: self.kind,
        })
    }
}

/// A transition builder omitted a required field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TransitionBuildError {
    /// The source state was not supplied.
    #[error("transition source state is missing")]
    MissingSource,
    /// The triggering event was not supplied.
    #[error("transition event is missing")]
    MissingEvent,
    /// The destination state was not supplied.
    #[error("transition destination state is missing")]
    MissingDestination,
    /// The transition reached its effect-intent bound.
    #[error("transition effect count exceeds limit 16")]
    EffectLimitExceeded,
    /// The transition reached its action bound.
    #[error("transition action count exceeds limit 16")]
    ActionLimitExceeded,
    /// An internal transition must target its own source state.
    #[error("internal transition must be a self-transition")]
    InternalTransitionMustBeSelf,
    /// The transition reached its postcondition bound.
    #[error("transition postcondition count exceeds limit 16")]
    PostconditionLimitExceeded,
    /// The transition reached its output-event bound.
    #[error("transition output-event count exceeds limit 16")]
    OutputEventLimitExceeded,
}

/// Self-transition action semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionKind {
    /// Run source exits, transition actions, and destination entries.
    External,
    /// Run only transition actions; valid only when source and destination
    /// match.
    Internal,
}
