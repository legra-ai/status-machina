use super::{
    BehaviorUse,
    RecordBuildError,
};
use crate::{
    BehaviorRef,
    BehaviorRegistry,
    DefaultIds,
    MachineIds,
    StateDefinition,
};

/// Borrowed state data. All behavior lists use the native per-phase bounds.
/// This boundary record is not an executable or validated state.
#[derive(Clone, Debug)]
pub struct StateRecord<'a, I: MachineIds = DefaultIds> {
    /// State identity.
    pub id: I::State,
    /// Whether outgoing transitions are forbidden.
    pub terminal: bool,
    /// Optional selected invariant implementation.
    pub invariant: Option<BehaviorRef<I>>,
    /// Entry actions in execution order.
    pub entry_actions: &'a [BehaviorUse<I::Action, I>],
    /// Exit actions in execution order.
    pub exit_actions: &'a [BehaviorUse<I::Action, I>],
    /// Entry effect constructors in emission order.
    pub entry_effects: &'a [BehaviorUse<I::Effect, I>],
    /// Exit effect constructors in emission order.
    pub exit_effects: &'a [BehaviorUse<I::Effect, I>],
}
impl<I: MachineIds> StateRecord<'_, I> {
    /// Describe a nonterminal state without behaviors.
    pub const fn new(id: I::State) -> Self {
        Self {
            id,
            terminal: false,
            invariant: None,
            entry_actions: &[],
            exit_actions: &[],
            entry_effects: &[],
            exit_effects: &[],
        }
    }
    /// Declare a terminal state.
    #[must_use]
    pub const fn terminal(mut self) -> Self {
        self.terminal = true;
        self
    }
    pub(super) fn bind<D, E, F, X, O>(
        &self,
        registry: &BehaviorRegistry<D, E, F, X, O, I>,
    ) -> Result<StateDefinition<D, E, F, X, I>, RecordBuildError<I>> {
        let mut builder = StateDefinition::builder(self.id.clone());
        if self.terminal {
            builder = builder.terminal();
        }
        if let Some(reference) = &self.invariant {
            builder = builder.bound_invariant(registry.resolve_invariant(reference)?);
        }
        for usage in self.entry_actions {
            builder = builder
                .entry_action(registry.resolve_action(usage.id.clone(), &usage.reference)?)?;
        }
        for usage in self.exit_actions {
            builder = builder
                .exit_action(registry.resolve_action(usage.id.clone(), &usage.reference)?)?;
        }
        for usage in self.entry_effects {
            builder = builder
                .entry_effect(registry.resolve_effect(usage.id.clone(), &usage.reference)?)?;
        }
        for usage in self.exit_effects {
            builder = builder
                .exit_effect(registry.resolve_effect(usage.id.clone(), &usage.reference)?)?;
        }
        Ok(builder.build())
    }
}
