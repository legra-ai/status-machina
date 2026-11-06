//! State effects are bounded caller-owned intents, never executed by the
//! kernel.
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use state_machine::{
    ActionId,
    Condition,
    ConditionId,
    DefinitionBuildError,
    DispatchError,
    DispatchOutcome,
    Effect,
    EffectId,
    Event,
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

fn state() -> StateId {
    StateId::try_new("idle").unwrap()
}
fn event() -> Event<()> {
    Event::new(EventId::try_new("tick").unwrap(), ())
}
fn identity() -> MachineInstanceId {
    MachineInstanceId::try_new(uuid::Uuid::from_u128(
        0x0000_0000_0000_7000_8000_0000_0000_0001,
    ))
    .unwrap()
}
fn effect(tag: u8) -> Effect<u64, (), (), u8> {
    Effect::builder(EffectId::try_new(format!("effect-{tag}")).unwrap())
        .describe(move |_: &u64, _: &(), _: &()| tag)
        .build()
        .unwrap()
}
fn definition(internal: bool) -> MachineDefinition<u64, (), (), u8> {
    let mut transition = Transition::builder_with_effects(TransitionId::try_new("tick").unwrap())
        .from(state())
        .on(event().id().clone())
        .to(state())
        .effect(effect(2))
        .unwrap();
    if internal {
        transition = transition.internal();
    }
    MachineDefinition::builder_with_effects(MachineId::try_new("state-effects").unwrap())
        .initial(state())
        .state(
            StateDefinition::builder(state())
                .exit_effect(effect(1))
                .unwrap()
                .entry_effect(effect(3))
                .unwrap()
                .build(),
        )
        .unwrap()
        .transition(transition.build().unwrap())
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn external_self_transition_returns_exit_transition_entry_effects_in_order() {
    let definition = definition(false);
    let mut instance = definition.instantiate(identity(), 0).unwrap();
    let decision = instance.evaluate(event(), &()).unwrap();
    assert_eq!(instance.revision(), 0);
    let DispatchOutcome::Transitioned { effects, .. } = instance.accept(decision).unwrap() else {
        panic!("expected transition")
    };
    assert_eq!(effects.len(), 3);
    for (intent, expected) in effects.into_iter().zip([1, 2, 3]) {
        assert_eq!(intent.into_parts().1, expected);
    }
}

#[test]
fn internal_self_transition_returns_only_transition_effects() {
    let definition = definition(true);
    let mut instance = definition.instantiate(identity(), 0).unwrap();
    let DispatchOutcome::Transitioned { effects, .. } = instance.dispatch(event(), &()).unwrap()
    else {
        panic!("expected transition")
    };
    assert_eq!(effects.len(), 1);
    assert_eq!(effects.into_iter().next().unwrap().into_parts().1, 2);
}

#[test]
fn construction_initialization_drop_and_rejected_candidates_construct_no_effects() {
    for failure in ["action", "postcondition", "invariant"] {
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = |tag| {
            let calls = Arc::clone(&calls);
            Effect::builder(EffectId::try_new(tag).unwrap())
                .describe(move |_: &u64, _: &(), _: &()| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    0_u8
                })
                .build()
                .unwrap()
        };
        let transition = Transition::builder_with_effects(TransitionId::try_new("tick").unwrap())
            .from(state())
            .on(event().id().clone())
            .to(state())
            .action(
                LocalAction::builder(ActionId::try_new("change").unwrap())
                    .apply(move |data: &mut u64, _: &(), _: &()| {
                        *data = 1;
                        if failure == "action" {
                            Err(String::from("reject"))
                        } else {
                            Ok(())
                        }
                    })
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .postcondition(
                Condition::builder(ConditionId::try_new("check").unwrap())
                    .evaluate(move |_: &u64, _: &(), _: &()| failure != "postcondition")
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .effect(counted("transition"))
            .unwrap()
            .build()
            .unwrap();
        let definition = Arc::new(
            MachineDefinition::<u64, (), (), u8>::builder_with_effects(
                MachineId::try_new("reject").unwrap(),
            )
            .initial(state())
            .state(
                StateDefinition::builder(state())
                    .invariant(move |data| failure != "invariant" || *data == 0)
                    .entry_effect(counted("entry"))
                    .unwrap()
                    .exit_effect(counted("exit"))
                    .unwrap()
                    .build(),
            )
            .unwrap()
            .transition(transition)
            .unwrap()
            .build()
            .unwrap(),
        );
        let mut instance = Arc::clone(&definition)
            .instantiate_shared(identity(), 0)
            .unwrap();
        let borrowed = definition.instantiate(identity(), 0).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let result = instance.dispatch(event(), &());
        assert!(matches!(
            result,
            Err(DispatchError::ActionFailed { .. }
                | DispatchError::PostconditionFailed { .. }
                | DispatchError::InvariantFailed { .. })
        ));
        assert_eq!(*instance.data(), 0);
        assert_eq!(instance.revision(), 0);
        drop(borrowed);
        drop(instance);
        drop(definition);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn state_effect_bounds_limit_each_phase_and_total_dispatch() {
    for entry in [true, false] {
        let mut builder = StateDefinition::<u64, (), (), u8>::builder(state());
        for tag in 0..16 {
            builder = if entry {
                builder.entry_effect(effect(tag)).unwrap()
            } else {
                builder.exit_effect(effect(tag)).unwrap()
            };
        }
        let result = if entry {
            builder.entry_effect(effect(16))
        } else {
            builder.exit_effect(effect(16))
        };
        assert!(matches!(
            result,
            Err(DefinitionBuildError::StateEffectLimitExceeded { .. })
        ));
    }
    let mut state_builder = StateDefinition::builder(state());
    let mut transition =
        Transition::builder_with_effects(TransitionId::try_new("bounded").unwrap())
            .from(state())
            .on(event().id().clone())
            .to(state());
    for tag in 0..16 {
        state_builder = state_builder
            .exit_effect(effect(tag))
            .unwrap()
            .entry_effect(effect(tag + 32))
            .unwrap();
        transition = transition.effect(effect(tag + 16)).unwrap();
    }
    let definition = MachineDefinition::<u64, (), (), u8>::builder_with_effects(
        MachineId::try_new("bounded").unwrap(),
    )
    .initial(state())
    .state(state_builder.build())
    .unwrap()
    .transition(transition.build().unwrap())
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition.instantiate(identity(), 0).unwrap();
    let DispatchOutcome::Transitioned { effects, .. } = instance.dispatch(event(), &()).unwrap()
    else {
        panic!("expected transition")
    };
    assert_eq!(effects.len(), 48);
    for (intent, tag) in effects.into_iter().zip(0..48) {
        assert_eq!(intent.into_parts().1, tag);
    }
}

#[path = "state_effects/phases.rs"]
mod phases;
