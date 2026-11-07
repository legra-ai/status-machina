use super::{
    DispositionRecord,
    RecordBuildResult,
    StateRecord,
    TransitionDefinitionRecord,
};
use crate::{
    BehaviorRegistry,
    CandidateData,
    DefaultIds,
    MachineDefinition,
    MachineIds,
};

/// Data-only borrowed view of a machine graph, independent of serialization.
///
/// No callbacks or payload values occur in this model. Binding traverses
/// borrowed records and retains only the bounded executable graph. Format
/// readers must bound their own input storage; this view does not allocate a
/// second graph. Slice order is behavior declaration order, not lifecycle
/// display order.
#[derive(Clone, Debug)]
pub struct DefinitionRecord<'a, I: MachineIds = DefaultIds> {
    /// Definition identity, independent of its format or external IRI.
    pub id: I::Definition,
    /// Explicit initial state; never inferred from labels or display order.
    pub initial: I::State,
    /// Positive maximum state count.
    pub max_states: usize,
    /// Positive maximum transition count.
    pub max_transitions: usize,
    /// State declarations.
    pub states: &'a [StateRecord<'a, I>],
    /// Transition declarations.
    pub transitions: &'a [TransitionDefinitionRecord<'a, I>],
    /// Harmless event dispositions, subject to the native cap.
    pub dispositions: &'a [DispositionRecord<'a, I>],
}
impl<'a, I: MachineIds> DefinitionRecord<'a, I> {
    /// Describe a graph with native default limits and no dispositions.
    pub const fn new(
        id: I::Definition,
        initial: I::State,
        states: &'a [StateRecord<'a, I>],
        transitions: &'a [TransitionDefinitionRecord<'a, I>],
    ) -> Self {
        Self {
            id,
            initial,
            states,
            transitions,
            max_states: crate::definition::DEFAULT_MAX_STATES,
            max_transitions: crate::definition::DEFAULT_MAX_TRANSITIONS,
            dispositions: &[],
        }
    }
    /// Resolve every behavior and validate through the native builders.
    ///
    /// No callbacks run. The returned graph owns its identities and callbacks;
    /// neither these records nor the registry need outlive it.
    ///
    /// # Errors
    /// Rejects unknown or mismatched bindings and all native builder
    /// violations.
    pub fn bind<D: CandidateData, E, F, X, O>(
        &self,
        registry: &BehaviorRegistry<D, E, F, X, O, I>,
    ) -> RecordBuildResult<MachineDefinition<D, E, F, X, O, I>, I> {
        let mut builder =
            MachineDefinition::builder_with_effects_and_output_events(self.id.clone())
                .initial(self.initial.clone())
                .limits(self.max_states, self.max_transitions)?;
        for state in self.states {
            builder = builder.state(state.bind(registry)?)?;
        }
        for transition in self.transitions {
            builder = builder.transition(transition.bind(registry)?)?;
        }
        for disposition in self.dispositions {
            builder = builder.disposition(disposition.bind(registry)?)?;
        }
        Ok(builder.build()?)
    }
}
