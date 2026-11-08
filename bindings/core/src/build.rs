//! One checked construction path into the Rust kernel.
use std::sync::Arc;

use status_machina::{
    BehaviorRef,
    BehaviorRegistry,
    Capability,
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
    Host,
    Value,
};
use crate::{
    callback,
    spec,
};

pub type Definition<H> = MachineDefinition<Data<H>, Value<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;
pub type NativeCapability<H> = Capability<Data<H>, Value<H>, Ids<H>>;
pub type Capabilities<H> = Arc<Vec<(u32, NativeCapability<H>)>>;
pub type Registry<H> = BehaviorRegistry<Data<H>, Value<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;
type NativeState<H> = StateDefinition<Data<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;
type NativeTransition<H> = Transition<Data<H>, Value<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;
type NativeCondition<H> = Condition<Data<H>, Value<H>, Value<H>, Ids<H>>;
type NativeAction<H> = LocalAction<Data<H>, Value<H>, Value<H>, Ids<H>>;
type NativeEffect<H> = Effect<Data<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;
type NativeOutput<H> = OutputEvent<Data<H>, Value<H>, Value<H>, Value<H>, Ids<H>>;

/// Immutable executable graph plus its checked Story implementation registry.
pub struct Template<H: Host> {
    pub definition: Arc<Definition<H>>,
    pub registry: Registry<H>,
    pub capabilities: Capabilities<H>,
    pub labels: crate::inspection::LabelInventory,
}
impl<H: Host> Template<H> {
    pub fn parse(source: &str) -> Result<Self, Error<H>> {
        if source.len() > 1_048_576 {
            return Err(Error::construction(
                "definition",
                "specification exceeds 1 MiB",
            ));
        }
        let spec: spec::Definition =
            serde_json::from_str(source).map_err(|e| Error::construction("specification", e))?;
        let labels = crate::inspection::LabelInventory::from_spec(&spec);
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
        if spec.capabilities.len() > 4096 {
            return Err(Error::construction(
                "capability",
                "inventory exceeds 4096 entries",
            ));
        }
        // bounded: private specification bytes and the 4096 capability limit.
        let mut capabilities = Vec::with_capacity(spec.capabilities.len());
        for spec in spec.capabilities {
            if capabilities.iter().any(|(id, _)| *id == spec.id) {
                return Err(Error::construction("capability", "duplicate identifier"));
            }
            let id = spec.id;
            let capability = match spec.implementation.select()? {
                Selection::Inline(cb) => Capability::builder(id)
                    .try_evaluate(move |s, d, f| callback::capability(cb, s, d, f))
                    .build()
                    .map_err(|e| Error::construction("capability", e))?,
                Selection::Registered(r) => registry
                    .resolve_capability(id, &r)
                    .map_err(|e| Error::construction("capability binding", e))?,
            };
            capabilities.push((id, capability));
        }
        Ok(Self {
            capabilities: Arc::new(capabilities),
            labels,
            definition: Arc::new(definition.build()?),
            registry,
        })
    }
    fn registry(behaviors: Vec<spec::Behavior>, limit: usize) -> Result<Registry<H>, Error<H>> {
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
pub(crate) fn reference<H: Host>(value: spec::Reference) -> BehaviorRef<Ids<H>> {
    BehaviorRef::new(value.story, value.implementation)
}
pub(crate) struct Builder<'a, H: Host> {
    pub registry: &'a Registry<H>,
}
impl<H: Host> Builder<'_, H> {
    fn state(&self, spec: spec::State) -> Result<NativeState<H>, Error<H>> {
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
    fn transition(&self, spec: spec::Transition) -> Result<NativeTransition<H>, Error<H>> {
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
    pub fn condition(&self, spec: spec::Condition) -> Result<NativeCondition<H>, Error<H>> {
        let is_all = matches!(&spec.expression, spec::Expression::All { .. });
        match spec.expression {
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
        }
    }
    fn action(&self, spec: spec::Named) -> Result<NativeAction<H>, Error<H>> {
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
    fn effect(&self, spec: spec::Named) -> Result<NativeEffect<H>, Error<H>> {
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
    fn output(&self, spec: spec::Named) -> Result<NativeOutput<H>, Error<H>> {
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
pub(crate) enum Selection<H: Host> {
    Inline(u32),
    Registered(BehaviorRef<Ids<H>>),
}
impl spec::Binding {
    pub(crate) fn select<H: Host>(self) -> Result<Selection<H>, Error<H>> {
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
