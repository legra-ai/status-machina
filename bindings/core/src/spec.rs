//! Checked private definition transport; identifiers are role-specific indexes.
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub callback: Option<u32>,
    pub reference: Option<Reference>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub story: u32,
    pub implementation: u32,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub id: u32,
    pub implementation: Binding,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub id: u32,
    pub expression: Expression,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expression {
    Predicate { implementation: Binding },
    All { children: Vec<Condition> },
    Any { children: Vec<Condition> },
    Not { child: Box<Condition> },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub id: u32,
    pub terminal: bool,
    pub invariant: Option<Binding>,
    pub entry_actions: Vec<Named>,
    pub exit_actions: Vec<Named>,
    pub entry_effects: Vec<Named>,
    pub exit_effects: Vec<Named>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub id: u32,
    pub source: u32,
    pub event: u32,
    pub target: u32,
    pub internal: bool,
    pub guard: Option<Condition>,
    pub postconditions: Vec<Condition>,
    pub actions: Vec<Named>,
    pub effects: Vec<Named>,
    pub outputs: Vec<Named>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disposition {
    pub id: u32,
    pub source: u32,
    pub event: u32,
    pub condition: Condition,
    pub reason: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Behavior {
    pub reference: Reference,
    pub role: Role,
    pub callback: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Action,
    Condition,
    Invariant,
    Effect,
    Output,
    Capability,
    Assertion,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub initial: u32,
    pub max_states: usize,
    pub max_transitions: usize,
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
    pub dispositions: Vec<Disposition>,
    pub behaviors: Vec<Behavior>,
    pub max_bindings: usize,
}
