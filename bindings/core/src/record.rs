//! Portable records bind through the kernel's own `DefinitionRecord::bind`.
//!
//! The private transport is the ordinary checked specification, restricted to
//! exact Story references: a record never carries an inline callback, so no
//! code is selected except by the explicit registry.
use std::sync::Arc;

use status_machina::{
    BehaviorRef,
    BehaviorUse,
    ConditionRecord,
    DefinitionRecord,
    DispositionRecord,
    ForbiddenEdge,
    StateRecord,
    TransitionDefinitionRecord,
    TransitionKind,
};
use typed_arena::Arena;

use crate::build::{
    Registry,
    Selection,
    Template,
};
use crate::error::Error;
use crate::ids::Ids;
use crate::spec;
use crate::value::Host;

type Use<H> = BehaviorUse<u32, Ids<H>>;
type Condition<'a, H> = ConditionRecord<'a, Ids<H>>;

/// Borrowed storage for one record view; dropped as soon as binding returns.
struct Storage<'a, H: Host> {
    uses: Arena<Use<H>>,
    conditions: Arena<Condition<'a, H>>,
    reasons: Arena<String>,
}
impl<'a, H: Host> Storage<'a, H> {
    fn new() -> Self {
        Self {
            uses: Arena::new(),
            conditions: Arena::new(),
            reasons: Arena::new(),
        }
    }
    fn reference(
        binding: spec::Binding,
        role: &'static str,
    ) -> Result<BehaviorRef<Ids<H>>, Error<H>> {
        match binding.select()? {
            Selection::Registered(reference) => Ok(reference),
            Selection::Inline(_) => Err(Error::construction(
                role,
                "portable records select Story implementations by reference, never by callback",
            )),
        }
    }
    fn uses(
        &'a self,
        values: Vec<spec::Named>,
        role: &'static str,
    ) -> Result<&'a [Use<H>], Error<H>> {
        // bounded: the transport is capped at 1 MiB and each phase at 16
        // natively.
        let mut lowered = Vec::with_capacity(values.len());
        for value in values {
            lowered.push(BehaviorUse::new(
                value.id,
                Self::reference(value.implementation, role)?,
            ));
        }
        Ok(self.uses.alloc_extend(lowered))
    }
    fn condition(&'a self, value: spec::Condition) -> Result<Condition<'a, H>, Error<H>> {
        let id = value.id;
        Ok(match value.expression {
            spec::Expression::Predicate { implementation } => ConditionRecord::Predicate(
                BehaviorUse::new(id, Self::reference(implementation, "predicate")?),
            ),
            spec::Expression::All { children } => ConditionRecord::All {
                id,
                operands: self.operands(children)?,
            },
            spec::Expression::Any { children } => ConditionRecord::Any {
                id,
                operands: self.operands(children)?,
            },
            spec::Expression::Not { child } => ConditionRecord::Not {
                id,
                operand: self.conditions.alloc(self.condition(*child)?),
            },
        })
    }
    fn operands(
        &'a self,
        values: Vec<spec::Condition>,
    ) -> Result<&'a [Condition<'a, H>], Error<H>> {
        // bounded: operands are capped at 16 by the native record constructors.
        let mut lowered = Vec::with_capacity(values.len());
        for value in values {
            lowered.push(self.condition(value)?);
        }
        Ok(self.conditions.alloc_extend(lowered))
    }
    fn single(&'a self, value: spec::Condition) -> Result<&'a Condition<'a, H>, Error<H>> {
        let condition = self.condition(value)?;
        Ok(self.conditions.alloc(condition))
    }
    fn state(&'a self, value: spec::State) -> Result<StateRecord<'a, Ids<H>>, Error<H>> {
        Ok(StateRecord {
            id: value.id,
            terminal: value.terminal,
            invariant: value
                .invariant
                .map(|binding| Self::reference(binding, "invariant"))
                .transpose()?,
            entry_actions: self.uses(value.entry_actions, "entry action")?,
            exit_actions: self.uses(value.exit_actions, "exit action")?,
            entry_effects: self.uses(value.entry_effects, "entry effect")?,
            exit_effects: self.uses(value.exit_effects, "exit effect")?,
        })
    }
    fn transition(
        &'a self,
        value: spec::Transition,
    ) -> Result<TransitionDefinitionRecord<'a, Ids<H>>, Error<H>> {
        // bounded: postconditions are capped at 16 by the native constructors.
        let mut postconditions = Vec::with_capacity(value.postconditions.len());
        for condition in value.postconditions {
            postconditions.push(self.condition(condition)?);
        }
        Ok(TransitionDefinitionRecord {
            id: value.id,
            from: value.source,
            event: value.event,
            to: value.target,
            kind: if value.internal {
                TransitionKind::Internal
            } else {
                TransitionKind::External
            },
            guard: value.guard.map(|guard| self.single(guard)).transpose()?,
            postconditions: self.conditions.alloc_extend(postconditions),
            actions: self.uses(value.actions, "action")?,
            effects: self.uses(value.effects, "effect")?,
            output_events: self.uses(value.outputs, "output")?,
        })
    }
    fn disposition(
        &'a self,
        value: spec::Disposition,
    ) -> Result<DispositionRecord<'a, Ids<H>>, Error<H>> {
        Ok(DispositionRecord {
            id: value.id,
            from: value.source,
            event: value.event,
            condition: self.single(value.condition)?,
            reason: self.reasons.alloc(value.reason).as_str(),
        })
    }
}
impl<H: Host> Template<H> {
    /// Bind a reference-only portable record against its explicit registry.
    pub fn bind_record(source: &str) -> Result<Self, Error<H>> {
        let mut spec = Self::read(source)?;
        if !spec.capabilities.is_empty() {
            return Err(Error::construction(
                "record",
                "portable definition records do not declare capabilities",
            ));
        }
        let labels = crate::inspection::LabelInventory::from_spec(&spec);
        let registry = Self::registry(std::mem::take(&mut spec.behaviors), spec.max_bindings)?;
        let definition = Self::bind_view(&registry, spec)?;
        Ok(Self {
            definition: Arc::new(definition),
            registry,
            capabilities: Arc::new(Vec::new()),
            labels,
        })
    }
    fn bind_view(
        registry: &Registry<H>,
        spec: spec::Definition,
    ) -> Result<crate::build::Definition<H>, Error<H>> {
        let storage = Storage::<H>::new();
        // bounded: each inventory is capped by the transport size and native
        // limits.
        let mut state_records = Vec::with_capacity(spec.states.len());
        for state in spec.states {
            state_records.push(storage.state(state)?);
        }
        let mut transition_records = Vec::with_capacity(spec.transitions.len());
        for transition in spec.transitions {
            transition_records.push(storage.transition(transition)?);
        }
        let mut disposition_records = Vec::with_capacity(spec.dispositions.len());
        for disposition in spec.dispositions {
            disposition_records.push(storage.disposition(disposition)?);
        }
        // bounded: the transport is capped at 1 MiB; the kernel rejects more
        // than its fixed prohibition bound before binding anything.
        let forbidden_records: Vec<ForbiddenEdge<Ids<H>>> = spec
            .forbidden_edges
            .iter()
            .map(|edge| edge.lower())
            .collect();
        let record = DefinitionRecord {
            id: uuid::Uuid::now_v7(),
            initial: spec.initial,
            max_states: spec.max_states,
            max_transitions: spec.max_transitions,
            states: &state_records,
            transitions: &transition_records,
            dispositions: &disposition_records,
            forbidden_edges: &forbidden_records,
        };
        record.bind(registry).map_err(Error::Record)
    }
}
