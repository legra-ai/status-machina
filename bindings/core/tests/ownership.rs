//! Compact native references remain bounded and follow actual Rust ownership.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use status_machina::{
    EvaluationError,
    Event,
};
use status_machina_bridge::build::Template;
use status_machina_bridge::callback::{
    Request,
    Response,
};
use status_machina_bridge::value::{
    ArenaState,
    Data,
    Host,
    Value,
};

struct NativeHost {
    arena: ArenaState,
    next: Cell<u32>,
}
impl Host for NativeHost {
    fn arena(&self) -> &ArenaState {
        &self.arena
    }
    fn execute(&self, _: Request) -> Result<Response, EvaluationError> {
        let key = self.next.get();
        self.next.set(key + 1);
        Ok(Response::Value(key))
    }
}
fn host(limit: usize) -> Rc<dyn Host> {
    Rc::new(NativeHost {
        arena: ArenaState::new(limit).unwrap(),
        next: Cell::new(10),
    })
}
const SPEC: &str = r#"{
 "initial":0,"max_states":128,"max_transitions":512,"max_bindings":128,
 "states":[
  {"id":0,"terminal":false,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]},
  {"id":1,"terminal":true,"invariant":null,"entry_actions":[],"exit_actions":[],"entry_effects":[],"exit_effects":[]}
 ],
 "transitions":[{"id":0,"source":0,"event":0,"target":1,"internal":false,"guard":null,
 "postconditions":[],"actions":[],"effects":[],"outputs":[]}],"dispositions":[],"behaviors":[]
}"#;
#[test]
fn value_leases_are_released_once_after_the_last_clone() {
    let host = host(1);
    let value = Value::claim(Rc::clone(&host), 0).unwrap();
    let clone = value.clone();
    drop(value);
    assert_eq!(host.arena().live(), [0]);
    assert!(Value::claim(Rc::clone(&host), 1).is_err());
    drop(clone);
    assert!(host.arena().live().is_empty());
    assert!(Value::claim(Rc::clone(&host), 1).is_ok());
}
#[test]
fn native_specification_uses_the_kernel_unknown_state_validation() {
    let invalid = SPEC.replace("\"target\":1", "\"target\":2");
    assert!(matches!(
        Template::parse(&invalid),
        Err(status_machina_bridge::error::Error::Definition(
            status_machina::DefinitionBuildError::UnknownTransitionState { state: 2, .. }
        ))
    ));
}
#[test]
fn native_dispatch_uses_kernel_acceptance_and_releases_discarded_candidates() {
    let template = Template::parse(SPEC).unwrap();
    let host = host(8);
    let mut instance = Arc::clone(&template.definition)
        .instantiate_shared(
            uuid::Uuid::now_v7(),
            Data {
                value: Value::claim(Rc::clone(&host), 0).unwrap(),
            },
        )
        .unwrap();
    let event = Value::claim(Rc::clone(&host), 1).unwrap();
    let facts = Value::claim(Rc::clone(&host), 2).unwrap();
    assert!(
        instance
            .dispatch(Event::new(0, event), &facts)
            .unwrap()
            .is_transitioned()
    );
    assert_eq!(*instance.state(), 1);
    assert_eq!(instance.revision(), 1);
    assert_eq!(instance.data().value.key().key, 11);
    let mut live = host.arena().live();
    live.sort_unstable();
    assert_eq!(live, [2, 11]);
    drop(facts);
    drop(instance);
    assert!(host.arena().live().is_empty());
}

#[test]
fn native_decision_is_unaccepted_until_checked_acceptance() {
    let template = Template::parse(SPEC).unwrap();
    let host = host(16);
    let mut machine =
        status_machina_bridge::runtime::Machine::try_new(&template, Rc::clone(&host), 0).unwrap();
    let decision = machine.evaluate(0, 1, 2, None).unwrap();
    assert_eq!(machine.state().unwrap(), 0);
    assert_eq!(machine.revision().unwrap(), 0);
    assert_eq!(decision.expected_revision(), 0);
    let outcome = machine.accept(decision).unwrap();
    assert!(matches!(
        outcome,
        status_machina_bridge::runtime::Outcome::Transitioned { .. }
    ));
    assert_eq!(machine.state().unwrap(), 1);
    assert_eq!(machine.revision().unwrap(), 1);
    machine.dispose();
    machine.dispose();
    assert!(machine.state().is_err());
}
