use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use state_machine::{
    ActionId,
    Effect,
    EffectId,
    Event,
    LocalAction,
    MachineInstanceId,
    OutputEvent,
};

use super::*;

fn identity(number: u128) -> MachineInstanceId {
    MachineInstanceId::try_new(uuid::Uuid::from_u128(
        0x0000_0000_0000_7000_8000_0000_0000_0000 + number,
    ))
    .unwrap()
}

#[test]
fn shared_definition_instances_keep_independent_data_state_and_revisions() {
    let transition = Transition::builder(TransitionId::try_new("advance").unwrap())
        .from(state("idle"))
        .on(EventId::try_new("tick").unwrap())
        .to(state("done"))
        .action(
            LocalAction::builder(ActionId::try_new("increment").unwrap())
                .apply(|data: &mut u64, _: &(), _: &()| {
                    *data += 1;
                    Ok(())
                })
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let definition = Arc::new(
        builder()
            .initial(state("idle"))
            .state(StateDefinition::builder(state("idle")).build())
            .unwrap()
            .state(StateDefinition::builder(state("done")).build())
            .unwrap()
            .transition(transition)
            .unwrap()
            .build()
            .unwrap(),
    );
    let mut first = Arc::clone(&definition)
        .instantiate_shared(identity(1), 11)
        .unwrap();
    let mut second = Arc::clone(&definition)
        .instantiate_shared(identity(2), 22)
        .unwrap();
    drop(definition);
    first
        .dispatch(Event::new(EventId::try_new("tick").unwrap(), ()), &())
        .unwrap();
    assert_eq!(first.state(), &state("done"));
    assert_eq!(first.revision(), 1);
    assert_eq!(*first.data(), 12);
    assert_eq!(second.state(), &state("idle"));
    assert_eq!(second.revision(), 0);
    assert_eq!(*second.data(), 22);
    second
        .dispatch(Event::new(EventId::try_new("tick").unwrap(), ()), &())
        .unwrap();
    assert_eq!(second.state(), &state("done"));
    assert_eq!(second.revision(), 1);
    assert_eq!(*second.data(), 23);
    assert_eq!(*first.data(), 12);
}

#[test]
fn construction_binds_callbacks_without_invoking_them() {
    let calls = Arc::new(AtomicUsize::new(0));
    let guard_calls = Arc::clone(&calls);
    let action_calls = Arc::clone(&calls);
    let effect_calls = Arc::clone(&calls);
    let output_calls = Arc::clone(&calls);
    let invariant_calls = Arc::clone(&calls);
    let definition =
        MachineDefinition::<u64, (), (), (), ()>::builder_with_effects_and_output_events(
            MachineId::try_new("pure-build").unwrap(),
        )
        .initial(state("idle"))
        .state(
            StateDefinition::builder(state("idle"))
                .invariant(move |_| {
                    invariant_calls.fetch_add(1, Ordering::SeqCst);
                    true
                })
                .build(),
        )
        .unwrap()
        .transition(
            Transition::builder_with_effects_and_output_events(
                TransitionId::try_new("tick").unwrap(),
            )
            .from(state("idle"))
            .on(EventId::try_new("tick").unwrap())
            .to(state("idle"))
            .when(
                Condition::builder(ConditionId::try_new("guard").unwrap())
                    .evaluate(move |_: &u64, _: &(), _: &()| {
                        guard_calls.fetch_add(1, Ordering::SeqCst);
                        true
                    })
                    .build()
                    .unwrap(),
            )
            .action(
                LocalAction::builder(ActionId::try_new("action").unwrap())
                    .apply(move |_: &mut u64, _: &(), _: &()| {
                        action_calls.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .effect(
                Effect::builder(EffectId::try_new("effect").unwrap())
                    .describe(move |_: &u64, _: &(), _: &()| {
                        effect_calls.fetch_add(1, Ordering::SeqCst);
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .output_event(
                OutputEvent::builder(EventId::try_new("output").unwrap())
                    .describe(move |_: &u64, _: &(), _: &()| {
                        output_calls.fetch_add(1, Ordering::SeqCst);
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .build()
            .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(definition);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
