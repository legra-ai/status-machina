use super::{
    BehaviorUse,
    ConditionRecord,
    RecordBuildError,
    RecordBuildResult,
};
use crate::{
    BehaviorRegistry,
    DefaultIds,
    EventDisposition,
    MachineIds,
    Transition,
    TransitionKind,
};

/// A permitted directed edge with explicit typed trigger and behavior
/// references. It describes a possible move, not a state-change occurrence.
#[derive(Clone, Debug)]
pub struct TransitionDefinitionRecord<'a, I: MachineIds = DefaultIds> {
    /// Edge identity.
    pub id: I::Transition,
    /// Source state identity.
    pub from: I::State,
    /// Typed executable event identity; not a human-readable trigger label.
    pub event: I::Event,
    /// Destination state identity.
    pub to: I::State,
    /// External or internal self-transition semantics.
    pub kind: TransitionKind,
    /// Optional guard expression.
    pub guard: Option<&'a ConditionRecord<'a, I>>,
    /// Candidate checks, bounded by the native builder.
    pub postconditions: &'a [ConditionRecord<'a, I>],
    /// Candidate actions in execution order, bounded by the native builder.
    pub actions: &'a [BehaviorUse<I::Action, I>],
    /// Effect constructors in emission order, bounded by the native builder.
    pub effects: &'a [BehaviorUse<I::Effect, I>],
    /// Output constructors in emission order, bounded by the native builder.
    pub output_events: &'a [BehaviorUse<I::Event, I>],
}
impl<I: MachineIds> TransitionDefinitionRecord<'_, I> {
    /// Describe an external transition with no optional behaviors.
    pub const fn new(id: I::Transition, from: I::State, event: I::Event, to: I::State) -> Self {
        Self {
            id,
            from,
            event,
            to,
            kind: TransitionKind::External,
            guard: None,
            postconditions: &[],
            actions: &[],
            effects: &[],
            output_events: &[],
        }
    }
    pub(super) fn bind<D, E, F, X, O>(
        &self,
        registry: &BehaviorRegistry<D, E, F, X, O, I>,
    ) -> RecordBuildResult<Transition<D, E, F, X, O, I>, I> {
        let mut builder = Transition::builder_with_effects_and_output_events(self.id.clone())
            .from(self.from.clone())
            .on(self.event.clone())
            .to(self.to.clone());
        if self.kind == TransitionKind::Internal {
            builder = builder.internal();
        }
        if let Some(guard) = self.guard {
            builder = builder.when(guard.bind(registry)?);
        }
        for condition in self.postconditions {
            builder = builder
                .postcondition(condition.bind(registry)?)
                .map_err(|source| self.error(source))?;
        }
        for usage in self.actions {
            builder = builder
                .action(registry.resolve_action(usage.id.clone(), &usage.reference)?)
                .map_err(|source| self.error(source))?;
        }
        for usage in self.effects {
            builder = builder
                .effect(registry.resolve_effect(usage.id.clone(), &usage.reference)?)
                .map_err(|source| self.error(source))?;
        }
        for usage in self.output_events {
            builder = builder
                .output_event(registry.resolve_output_event(usage.id.clone(), &usage.reference)?)
                .map_err(|source| self.error(source))?;
        }
        builder.build().map_err(|source| self.error(source))
    }
    fn error(&self, source: crate::TransitionBuildError) -> RecordBuildError<I> {
        RecordBuildError::Transition {
            id: self.id.clone(),
            source,
        }
    }
}

/// An explicitly harmless event disposition with a bound condition.
#[derive(Clone, Debug)]
pub struct DispositionRecord<'a, I: MachineIds = DefaultIds> {
    /// Disposition identity.
    pub id: I::Disposition,
    /// State where the disposition applies.
    pub from: I::State,
    /// Event it handles.
    pub event: I::Event,
    /// Required condition expression.
    pub condition: &'a ConditionRecord<'a, I>,
    /// Human-readable explanation; native validation limits it to 256 bytes.
    pub reason: &'a str,
}
impl<I: MachineIds> DispositionRecord<'_, I> {
    pub(super) fn bind<D, E, F, X, O>(
        &self,
        registry: &BehaviorRegistry<D, E, F, X, O, I>,
    ) -> Result<EventDisposition<D, E, F, I>, RecordBuildError<I>> {
        // Reject oversized borrowed input before the builder copies its text.
        if self.reason.len() > 256 {
            return Err(RecordBuildError::Disposition {
                id: self.id.clone(),
                source: crate::DispositionBuildError::InvalidReason,
            });
        }
        EventDisposition::builder(self.id.clone())
            .from(self.from.clone())
            .on(self.event.clone())
            .when(self.condition.bind(registry)?)
            .reason(self.reason)
            .build()
            .map_err(|source| RecordBuildError::Disposition {
                id: self.id.clone(),
                source,
            })
    }
}
