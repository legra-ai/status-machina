use std::sync::Arc;

use super::DefinitionBuildError;
use crate::{
    Effect,
    LocalAction,
};

type Invariant<D> = Arc<dyn Fn(&D) -> Result<bool, crate::EvaluationError> + Send + Sync>;

/// One named state and its optional data invariant.
pub struct StateDefinition<D, E, F, X = (), I: crate::MachineIds = crate::DefaultIds> {
    pub(super) id: I::State,
    invariant: Option<Invariant<D>>,
    invariant_binding: Option<crate::BehaviorRef<I>>,
    entry_actions: Vec<LocalAction<D, E, F, I>>,
    exit_actions: Vec<LocalAction<D, E, F, I>>,
    // bounded: builders cap each effect phase at 16.
    entry_effects: Vec<Effect<D, E, F, X, I>>,
    // bounded: builders cap each effect phase at 16.
    exit_effects: Vec<Effect<D, E, F, X, I>>,
    terminal: bool,
}

impl<D, E, F, X, I: crate::MachineIds> StateDefinition<D, E, F, X, I> {
    /// Start building a state.
    #[must_use]
    pub fn builder(id: I::State) -> StateDefinitionBuilder<D, E, F, X, I> {
        StateDefinitionBuilder {
            id,
            invariant: None,
            invariant_binding: None,
            entry_actions: Vec::new(),
            exit_actions: Vec::new(),
            entry_effects: Vec::new(),
            exit_effects: Vec::new(),
            terminal: false,
        }
    }

    /// Return the state identifier.
    #[must_use]
    pub const fn id(&self) -> &I::State {
        &self.id
    }

    pub(crate) fn bindings(&self) -> impl Iterator<Item = &crate::BehaviorRef<I>> {
        self.invariant_binding
            .iter()
            .chain(self.entry_actions.iter().filter_map(LocalAction::binding))
            .chain(self.exit_actions.iter().filter_map(LocalAction::binding))
            .chain(self.entry_effects.iter().filter_map(Effect::binding))
            .chain(self.exit_effects.iter().filter_map(Effect::binding))
    }

    pub(crate) fn accepts(&self, data: &D) -> Result<bool, crate::EvaluationError> {
        self.invariant
            .as_ref()
            .map_or(Ok(true), |invariant| invariant(data))
    }

    pub(crate) fn entry_actions(&self) -> &[LocalAction<D, E, F, I>] {
        &self.entry_actions
    }

    pub(crate) fn exit_actions(&self) -> &[LocalAction<D, E, F, I>] {
        &self.exit_actions
    }

    pub(crate) fn entry_effects(&self) -> &[Effect<D, E, F, X, I>] {
        &self.entry_effects
    }

    pub(crate) fn exit_effects(&self) -> &[Effect<D, E, F, X, I>] {
        &self.exit_effects
    }

    pub(crate) fn terminal(&self) -> bool {
        self.terminal
    }
}

/// Builder for a state definition.
pub struct StateDefinitionBuilder<D, E, F, X = (), I: crate::MachineIds = crate::DefaultIds> {
    id: I::State,
    invariant: Option<Invariant<D>>,
    invariant_binding: Option<crate::BehaviorRef<I>>,
    entry_actions: Vec<LocalAction<D, E, F, I>>,
    exit_actions: Vec<LocalAction<D, E, F, I>>,
    // bounded: builders cap each effect phase at 16.
    entry_effects: Vec<Effect<D, E, F, X, I>>,
    // bounded: builders cap each effect phase at 16.
    exit_effects: Vec<Effect<D, E, F, X, I>>,
    terminal: bool,
}

impl<D, E, F, X, I: crate::MachineIds> StateDefinitionBuilder<D, E, F, X, I> {
    /// Require the invariant to hold when an instance enters this state.
    #[must_use]
    pub fn invariant(mut self, invariant: impl Fn(&D) -> bool + Send + Sync + 'static) -> Self {
        self.invariant = Some(Arc::new(move |data| Ok(invariant(data))));
        self.invariant_binding = None;
        self
    }

    /// Require a state invariant that can report an application failure.
    #[must_use]
    pub fn try_invariant(
        mut self,
        invariant: impl Fn(&D) -> Result<bool, crate::EvaluationError> + Send + Sync + 'static,
    ) -> Self {
        self.invariant = Some(Arc::new(invariant));
        self
    }

    /// Use a named invariant resolved through a typed behavior registry.
    ///
    /// Construction records the reference and does not evaluate the invariant.
    #[must_use]
    pub fn bound_invariant(mut self, invariant: crate::BoundInvariant<D, I>) -> Self {
        let (binding, predicate) = invariant.into_parts();
        self.invariant = Some(predicate);
        self.invariant_binding = Some(binding);
        self
    }

    /// Add an entry action. Initial instantiation never runs entry actions.
    ///
    /// # Errors
    ///
    /// Returns an error after 16 entry actions.
    pub fn entry_action(
        mut self,
        action: LocalAction<D, E, F, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.entry_actions.len() == 16 {
            return Err(DefinitionBuildError::StateActionLimitExceeded {
                state: self.id.clone(),
            });
        }
        self.entry_actions.push(action);
        Ok(self)
    }

    /// Add an exit action.
    ///
    /// # Errors
    ///
    /// Returns an error after 16 exit actions.
    pub fn exit_action(
        mut self,
        action: LocalAction<D, E, F, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.exit_actions.len() == 16 {
            return Err(DefinitionBuildError::StateActionLimitExceeded {
                state: self.id.clone(),
            });
        }
        self.exit_actions.push(action);
        Ok(self)
    }

    /// Declare an entry effect intent for accepted external transitions.
    ///
    /// Instantiation never constructs these intents. Constructors observe the
    /// final validated candidate, after all actions and candidate checks.
    ///
    /// # Errors
    /// Rejects a seventeenth entry effect.
    pub fn entry_effect(
        mut self,
        effect: Effect<D, E, F, X, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.entry_effects.len() == 16 {
            return Err(DefinitionBuildError::StateEffectLimitExceeded {
                state: self.id.clone(),
            });
        }
        self.entry_effects.push(effect);
        Ok(self)
    }

    /// Declare an exit effect intent for accepted external transitions.
    ///
    /// Constructors observe the final validated candidate and execution remains
    /// with the caller. Internal self-transitions skip state effects.
    ///
    /// # Errors
    /// Rejects a seventeenth exit effect.
    pub fn exit_effect(
        mut self,
        effect: Effect<D, E, F, X, I>,
    ) -> Result<Self, DefinitionBuildError<I>> {
        if self.exit_effects.len() == 16 {
            return Err(DefinitionBuildError::StateEffectLimitExceeded {
                state: self.id.clone(),
            });
        }
        self.exit_effects.push(effect);
        Ok(self)
    }

    /// Mark this state terminal; it has no outgoing transitions.
    #[must_use]
    pub fn terminal(mut self) -> Self {
        self.terminal = true;
        self
    }

    /// Build the state definition.
    #[must_use]
    pub fn build(self) -> StateDefinition<D, E, F, X, I> {
        StateDefinition {
            id: self.id,
            invariant: self.invariant,
            invariant_binding: self.invariant_binding,
            entry_actions: self.entry_actions,
            exit_actions: self.exit_actions,
            entry_effects: self.entry_effects,
            exit_effects: self.exit_effects,
            terminal: self.terminal,
        }
    }
}
