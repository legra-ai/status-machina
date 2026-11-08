//! One checked construction path into the Rust kernel.
use std::sync::Arc;

use status_machina::{
    BehaviorRef,
    BehaviorRegistry,
    Condition,
    Effect,
    EventDisposition,
    LocalAction,
    MachineDefinition,
    OutputEvent,
    StateDefinition,
    Transition,
};

use crate::error::Error;
use crate::ids::Ids;
use crate::value::{
    Data,
    Value,
};
use crate::{
    callback,
    spec,
};

pub type Definition = MachineDefinition<Data, Value, Value, Value, Value, Ids>;
pub type Registry = BehaviorRegistry<Data, Value, Value, Value, Value, Ids>;
/// Immutable executable graph plus its checked Story implementation registry.
pub struct Template {
    pub definition: Arc<Definition>,
    pub registry: Registry,
}
impl Template {
    pub fn parse(source: &str) -> Result<Self, Error> {
        if source.len() > 1_048_576 {
            return Err(Error::construction(
                "definition",
                "specification exceeds 1 MiB",
            ));
        }
        let spec: spec::Definition =
            serde_json::from_str(source).map_err(|e| Error::construction("specification", e))?;
        let registry = Self::registry(spec.behaviors, spec.max_bindings)?;
        let build = Builder {
            registry: &registry,
        };
        let mut definition =
            Definition::builder_with_effects_and_output_events(uuid::Uuid::now_v7())
                .initial(spec.initial)
                .limits(spec.max_states, spec.max_transitions)?;
        for state in spec.states {
            definition = definition.state(build.state(state)?)?;
        }
        for transition in spec.transitions {
            definition = definition.transition(build.transition(transition)?)?;
        }
        for disposition in spec.dispositions {
            let d = EventDisposition::builder(disposition.id)
                .from(disposition.source)
                .on(disposition.event)
                .when(build.condition(disposition.condition)?)
                .reason(disposition.reason)
                .build()
                .map_err(|e| Error::construction("disposition", e))?;
            definition = definition.disposition(d)?;
        }
        Ok(Self {
            definition: Arc::new(definition.build()?),
            registry,
        })
    }
    fn registry(behaviors: Vec<spec::Behavior>, limit: usize) -> Result<Registry, Error> {
        let mut registry = Registry::builder()
            .max_bindings(limit)
            .map_err(|e| Error::construction("Story registry", e))?;
        for behavior in behaviors {
            let cb = behavior.callback;
            let reference = reference(behavior.reference);
            registry = match behavior.role {
                spec::Role::Action => {
                    registry.try_action(reference, move |d, e, f| callback::action(cb, d, e, f))
                }
                spec::Role::Condition => registry
                    .try_condition(reference, move |d, e, f| callback::predicate(cb, d, e, f)),
                spec::Role::Invariant => {
                    registry.try_invariant(reference, move |d| callback::invariant(cb, d))
                }
                spec::Role::Effect => {
                    registry.try_effect(reference, move |d, e, f| callback::construct(cb, d, e, f))
                }
                spec::Role::Output => registry
                    .try_output_event(reference, move |d, e, f| callback::construct(cb, d, e, f)),
                spec::Role::Capability => registry
                    .try_capability(reference, move |s, d, f| callback::capability(cb, s, d, f)),
                spec::Role::Assertion => {
                    registry.try_assertion(reference, move |o| callback::assertion(cb, o))
                }
            }
            .map_err(|e| Error::construction("Story registry", e))?;
        }
        Ok(registry.build())
    }
}
pub(crate) fn reference(value: spec::Reference) -> BehaviorRef<Ids> {
    BehaviorRef::new(value.story, value.implementation)
}
pub(crate) struct Builder<'a> {
    pub registry: &'a Registry,
}
impl Builder<'_> {
    fn state(
        &self,
        spec: spec::State,
    ) -> Result<StateDefinition<Data, Value, Value, Value, Ids>, Error> {
        let mut state = StateDefinition::builder(spec.id);
        if spec.terminal {
            state = state.terminal();
        }
        if let Some(binding) = spec.invariant {
            state = match binding.select()? {
                Selection::Inline(cb) => state.try_invariant(move |d| callback::invariant(cb, d)),
                Selection::Registered(r) => state.bound_invariant(
                    self.registry
                        .resolve_invariant(&r)
                        .map_err(|e| Error::construction("invariant binding", e))?,
                ),
            };
        }
        for action in spec.entry_actions {
            state = state
                .entry_action(self.action(action)?)
                .map_err(|e| Error::construction("entry action", e))?;
        }
        for action in spec.exit_actions {
            state = state
                .exit_action(self.action(action)?)
                .map_err(|e| Error::construction("exit action", e))?;
        }
        for effect in spec.entry_effects {
            state = state
                .entry_effect(self.effect(effect)?)
                .map_err(|e| Error::construction("entry effect", e))?;
        }
        for effect in spec.exit_effects {
            state = state
                .exit_effect(self.effect(effect)?)
                .map_err(|e| Error::construction("exit effect", e))?;
        }
        Ok(state.build())
    }
    fn transition(
        &self,
        spec: spec::Transition,
    ) -> Result<Transition<Data, Value, Value, Value, Value, Ids>, Error> {
        let mut transition = Transition::builder_with_effects_and_output_events(spec.id)
            .from(spec.source)
            .on(spec.event)
            .to(spec.target);
        if spec.internal {
            transition = transition.internal();
        }
        if let Some(condition) = spec.guard {
            transition = transition.when(self.condition(condition)?);
        }
        for condition in spec.postconditions {
            transition = transition
                .postcondition(self.condition(condition)?)
                .map_err(|e| Error::construction("postcondition", e))?;
        }
        for action in spec.actions {
            transition = transition
                .action(self.action(action)?)
                .map_err(|e| Error::construction("action", e))?;
        }
        for effect in spec.effects {
            transition = transition
                .effect(self.effect(effect)?)
                .map_err(|e| Error::construction("effect", e))?;
        }
        for output in spec.outputs {
            transition = transition
                .output_event(self.output(output)?)
                .map_err(|e| Error::construction("output", e))?;
        }
        transition
            .build()
            .map_err(|e| Error::construction("transition", e))
    }
    pub fn condition(
        &self,
        spec: spec::Condition,
    ) -> Result<Condition<Data, Value, Value, Ids>, Error> {
        let is_all = matches!(&spec.expression, spec::Expression::All { .. });
        let built = match spec.expression {
            spec::Expression::Predicate { implementation } => match implementation.select()? {
                Selection::Inline(cb) => Condition::builder(spec.id)
                    .try_evaluate(move |d, e, f| callback::predicate(cb, d, e, f))
                    .build()
                    .map_err(|e| Error::construction("predicate", e)),
                Selection::Registered(r) => self
                    .registry
                    .resolve_condition(spec.id, &r)
                    .map_err(|e| Error::construction("condition binding", e)),
            },
            spec::Expression::All { children } | spec::Expression::Any { children } => {
                let mut group = if is_all {
                    Condition::all(spec.id)
                } else {
                    Condition::any(spec.id)
                };
                for child in children {
                    group = group
                        .condition(self.condition(child)?)
                        .map_err(|e| Error::construction("condition group", e))?;
                }
                group
                    .build()
                    .map_err(|e| Error::construction("condition group", e))
            }
            spec::Expression::Not { child } => Condition::not(spec.id)
                .condition(self.condition(*child)?)
                .map_err(|e| Error::construction("negation", e))?
                .build()
                .map_err(|e| Error::construction("negation", e)),
        };
        built
    }
    fn action(&self, spec: spec::Named) -> Result<LocalAction<Data, Value, Value, Ids>, Error> {
        match spec.implementation.select()? {
            Selection::Inline(cb) => LocalAction::builder(spec.id)
                .try_apply(move |d, e, f| callback::action(cb, d, e, f))
                .build()
                .map_err(|e| Error::construction("action", e)),
            Selection::Registered(r) => self
                .registry
                .resolve_action(spec.id, &r)
                .map_err(|e| Error::construction("action binding", e)),
        }
    }
    fn effect(&self, spec: spec::Named) -> Result<Effect<Data, Value, Value, Value, Ids>, Error> {
        match spec.implementation.select()? {
            Selection::Inline(cb) => Effect::builder(spec.id)
                .try_describe(move |d, e, f| callback::construct(cb, d, e, f))
                .build()
                .map_err(|e| Error::construction("effect", e)),
            Selection::Registered(r) => self
                .registry
                .resolve_effect(spec.id, &r)
                .map_err(|e| Error::construction("effect binding", e)),
        }
    }
    fn output(
        &self,
        spec: spec::Named,
    ) -> Result<OutputEvent<Data, Value, Value, Value, Ids>, Error> {
        match spec.implementation.select()? {
            Selection::Inline(cb) => OutputEvent::builder(spec.id)
                .try_describe(move |d, e, f| callback::construct(cb, d, e, f))
                .build()
                .map_err(|e| Error::construction("output", e)),
            Selection::Registered(r) => self
                .registry
                .resolve_output_event(spec.id, &r)
                .map_err(|e| Error::construction("output binding", e)),
        }
    }
}
pub(crate) enum Selection {
    Inline(u32),
    Registered(BehaviorRef<Ids>),
}
impl spec::Binding {
    pub(crate) fn select(self) -> Result<Selection, Error> {
        match (self.callback, self.reference) {
            (Some(callback), None) => Ok(Selection::Inline(callback)),
            (None, Some(r)) => Ok(Selection::Registered(reference(r))),
            _ => Err(Error::construction(
                "callback",
                "choose exactly one inline callback or Story implementation reference",
            )),
        }
    }
}
