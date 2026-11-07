//! STM-01 behavior at the public, synchronous boundary.
use std::sync::Arc;

use status_machina::{
    ActionId,
    Condition,
    ConditionId,
    DefinitionBuildError,
    DispatchError,
    DispatchOutcome,
    DispositionId,
    Event,
    EventDisposition,
    EventId,
    LocalAction,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

#[path = "kernel_contract/bounds.rs"]
mod bounds;

fn state() -> StateId {
    StateId::try_new("idle").unwrap()
}
fn event() -> EventId {
    EventId::try_new("tick").unwrap()
}
fn instance_id() -> MachineInstanceId {
    MachineInstanceId::try_new(uuid::Uuid::from_u128(
        0x0000_0000_0000_7000_8000_0000_0000_0001,
    ))
    .unwrap()
}
fn action(name: &str, digit: u64) -> LocalAction<u64, (), ()> {
    LocalAction::builder(ActionId::try_new(name).unwrap())
        .apply(move |data: &mut u64, _: &(), _: &()| {
            *data = *data * 10 + digit;
            Ok(())
        })
        .build()
        .unwrap()
}
fn disposition(name: &str) -> EventDisposition<u64, (), ()> {
    EventDisposition::builder(DispositionId::try_new(name).unwrap())
        .from(state())
        .on(event())
        .when(
            Condition::builder(ConditionId::try_new(name).unwrap())
                .evaluate(|_: &u64, _: &(), _: &()| true)
                .build()
                .unwrap(),
        )
        .reason("explicit harmless event")
        .build()
        .unwrap()
}
fn self_definition(internal: bool) -> MachineDefinition<u64, (), ()> {
    let mut edge = Transition::builder(TransitionId::try_new("tick").unwrap())
        .from(state())
        .on(event())
        .to(state())
        .action(action("transition-one", 3))
        .unwrap()
        .action(action("transition-two", 4))
        .unwrap();
    if internal {
        edge = edge.internal();
    }
    MachineDefinition::builder(MachineId::try_new("neutral").unwrap())
        .initial(state())
        .state(
            StateDefinition::builder(state())
                .invariant(|data| *data < 1_000_000)
                .exit_action(action("exit-one", 1))
                .unwrap()
                .exit_action(action("exit-two", 2))
                .unwrap()
                .entry_action(action("entry-one", 5))
                .unwrap()
                .entry_action(action("entry-two", 6))
                .unwrap()
                .build(),
        )
        .unwrap()
        .transition(edge.build().unwrap())
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn borrowed_and_shared_initialization_validate_without_running_entry_actions() {
    let definition = Arc::new(self_definition(false));
    let borrowed = definition.instantiate(instance_id(), 0).unwrap();
    let shared = Arc::clone(&definition)
        .instantiate_shared(instance_id(), 0)
        .unwrap();
    for instance in [&borrowed, &shared] {
        assert_eq!(*instance.data(), 0);
        assert_eq!(instance.state(), &state());
        assert_eq!(instance.revision(), 0);
    }
    assert!(matches!(
        definition.instantiate(instance_id(), 1_000_000),
        Err(DefinitionBuildError::InitialInvariantFailed { .. })
    ));
    assert!(matches!(
        Arc::clone(&definition).instantiate_shared(instance_id(), 1_000_000),
        Err(DefinitionBuildError::InitialInvariantFailed { .. })
    ));
    drop(borrowed);
    drop(shared);
    drop(definition);
}

#[test]
fn self_transition_action_order_and_revision_match_selected_semantics() {
    for (internal, expected) in [(false, 123_456), (true, 34)] {
        let definition = self_definition(internal);
        let mut instance = definition.instantiate(instance_id(), 0).unwrap();
        assert!(
            instance
                .dispatch(Event::new(event(), ()), &())
                .unwrap()
                .is_transitioned()
        );
        assert_eq!(*instance.data(), expected);
        assert_eq!(instance.state(), &state());
        assert_eq!(instance.revision(), 1);
    }
}

#[test]
fn two_enabled_dispositions_are_ambiguous_without_mutation() {
    let definition =
        MachineDefinition::<u64, (), ()>::builder(MachineId::try_new("ambiguous").unwrap())
            .initial(state())
            .state(StateDefinition::builder(state()).build())
            .unwrap()
            .disposition(disposition("first"))
            .unwrap()
            .disposition(disposition("second"))
            .unwrap()
            .build()
            .unwrap();
    let mut instance = definition.instantiate(instance_id(), 7).unwrap();
    assert!(matches!(
        instance.dispatch(Event::new(event(), ()), &()),
        Err(DispatchError::Ambiguous { .. })
    ));
    assert_eq!(*instance.data(), 7);
    assert_eq!(instance.state(), &state());
    assert_eq!(instance.revision(), 0);
}

#[test]
fn terminal_state_refuses_unmatched_events_but_reports_explicit_dispositions() {
    let definition =
        MachineDefinition::<u64, (), ()>::builder(MachineId::try_new("terminal").unwrap())
            .initial(state())
            .state(StateDefinition::builder(state()).terminal().build())
            .unwrap()
            .disposition(disposition("harmless"))
            .unwrap()
            .build()
            .unwrap();
    let mut instance = definition.instantiate(instance_id(), 7).unwrap();
    assert!(matches!(
        instance.dispatch(Event::new(event(), ()), &()).unwrap(),
        DispatchOutcome::Ignored { .. }
    ));
    assert!(
        instance
            .dispatch(Event::new(EventId::try_new("unknown").unwrap(), ()), &())
            .unwrap()
            .is_refused()
    );
    assert_eq!(*instance.data(), 7);
    assert_eq!(instance.state(), &state());
    assert_eq!(instance.revision(), 0);
}

#[test]
fn stale_revision_is_rejected_before_guard_evaluation() {
    // A panic sentinel proves the stale path cannot reach this callback.
    let edge = Transition::builder(TransitionId::try_new("guarded").unwrap())
        .from(state())
        .on(event())
        .to(state())
        .when(
            Condition::builder(ConditionId::try_new("must-not-run").unwrap())
                .evaluate(|_: &u64, _: &(), _: &()| panic!("stale dispatch evaluated guard"))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();
    let definition =
        MachineDefinition::<u64, (), ()>::builder(MachineId::try_new("stale").unwrap())
            .initial(state())
            .state(StateDefinition::builder(state()).build())
            .unwrap()
            .transition(edge)
            .unwrap()
            .build()
            .unwrap();
    let mut instance = definition.instantiate(instance_id(), 7).unwrap();
    assert!(matches!(
        instance.dispatch_at(1, Event::new(event(), ()), &()),
        Err(DispatchError::StaleRevision {
            expected: 1,
            current: 0
        })
    ));
    assert_eq!(*instance.data(), 7);
    assert_eq!(instance.revision(), 0);
}
