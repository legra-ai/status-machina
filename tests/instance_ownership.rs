//! Shared-definition ownership regression for status-machina instances.

use std::sync::Arc;

use status_machina::{
    Event,
    EventId,
    MachineDefinition,
    MachineId,
    MachineInstance,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

fn state_id(value: &str) -> StateId {
    StateId::try_new(value).unwrap()
}

fn event_id(value: &str) -> EventId {
    EventId::try_new(value).unwrap()
}

fn definition() -> MachineDefinition<u8, (), ()> {
    let idle = state_id("idle");
    let complete = state_id("complete");
    let event = event_id("finish");
    let transition = Transition::builder(TransitionId::try_new("finish-transition").unwrap())
        .from(idle.clone())
        .on(event)
        .to(complete.clone())
        .build()
        .unwrap();
    MachineDefinition::<u8, (), ()>::builder(MachineId::try_new("ownership").unwrap())
        .initial(idle.clone())
        .state(StateDefinition::builder(idle).build())
        .unwrap()
        .state(StateDefinition::builder(complete).build())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build()
        .unwrap()
}

fn detached_instance() -> MachineInstance<'static, u8, (), ()> {
    let definition = Arc::new(definition());
    Arc::clone(&definition)
        .instantiate_shared(MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(), 0)
        .unwrap()
}

#[test]
fn instance_keeps_its_definition_after_the_callers_handle_is_dropped() {
    let mut instance = detached_instance();
    let outcome = instance
        .dispatch(Event::new(event_id("finish"), ()), &())
        .unwrap();

    assert!(outcome.is_transitioned());
    assert_eq!(instance.state().as_str(), "complete");
    assert_eq!(instance.revision(), 1);
}
