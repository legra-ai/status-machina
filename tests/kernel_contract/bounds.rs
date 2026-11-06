use state_machine::{
    Effect,
    EffectId,
    OutputEvent,
    TransitionBuildError,
};

use super::*;

fn edge() -> Transition<u64, (), ()> {
    Transition::builder(TransitionId::try_new("tick").unwrap())
        .from(state())
        .on(event())
        .to(state())
        .build()
        .unwrap()
}

#[test]
fn definition_rejects_excess_states_edges_and_reduced_limits() {
    let builder = || {
        MachineDefinition::<u64, (), ()>::builder(MachineId::try_new("bounded").unwrap())
            .limits(1, 1)
            .unwrap()
    };
    assert!(matches!(
        builder()
            .state(StateDefinition::builder(state()).build())
            .unwrap()
            .state(StateDefinition::builder(StateId::try_new("extra").unwrap()).build()),
        Err(DefinitionBuildError::StateLimitExceeded { limit: 1 })
    ));
    assert!(matches!(
        builder().transition(edge()).unwrap().transition(edge()),
        Err(DefinitionBuildError::TransitionLimitExceeded { limit: 1 })
    ));
    assert!(matches!(
        builder().limits(0, 1),
        Err(DefinitionBuildError::EmptyLimits)
    ));
    assert!(matches!(
        builder().limits(1, 0),
        Err(DefinitionBuildError::EmptyLimits)
    ));
    let two_states = builder()
        .limits(2, 1)
        .unwrap()
        .state(StateDefinition::builder(state()).build())
        .unwrap()
        .state(StateDefinition::builder(StateId::try_new("extra").unwrap()).build())
        .unwrap();
    assert!(matches!(
        two_states.limits(1, 1),
        Err(DefinitionBuildError::LimitsBelowCurrentSize)
    ));
}

#[test]
fn output_limits_bound_dispatch_and_preserve_declaration_order() {
    let mut edge = Transition::<u64, (), (), u8, u8>::builder_with_effects_and_output_events(
        TransitionId::try_new("outputs").unwrap(),
    )
    .from(state())
    .on(event())
    .to(state());
    for number in 0..16_u8 {
        edge = edge
            .effect(
                Effect::builder(EffectId::try_new(format!("effect-{number}")).unwrap())
                    .describe(move |_: &u64, _: &(), _: &()| number)
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .output_event(
                OutputEvent::builder(event())
                    .describe(move |_: &u64, _: &(), _: &()| number)
                    .build()
                    .unwrap(),
            )
            .unwrap();
    }
    let definition =
        MachineDefinition::<u64, (), (), u8, u8>::builder_with_effects_and_output_events(
            MachineId::try_new("bounded-output").unwrap(),
        )
        .initial(state())
        .state(StateDefinition::builder(state()).build())
        .unwrap()
        .transition(edge.build().unwrap())
        .unwrap()
        .build()
        .unwrap();
    let mut instance = definition.instantiate(instance_id(), 0).unwrap();
    let DispatchOutcome::Transitioned {
        effects, outputs, ..
    } = instance.dispatch(Event::new(event(), ()), &()).unwrap()
    else {
        panic!("expected accepted transition")
    };
    assert_eq!(effects.len(), 16);
    assert_eq!(outputs.len(), 16);
    for (number, (effect, output)) in (0..16_u8).zip(effects.iter().zip(&outputs)) {
        assert_eq!(effect.payload(), &number);
        assert_eq!(output.payload(), &number);
    }
    // Output events name the triggering event but are never recursively
    // dispatched.
    assert_eq!(instance.revision(), 1);
}

#[test]
fn seventeenth_effect_or_output_is_rejected_during_construction() {
    for effects in [true, false] {
        let mut edge = Transition::<u64, (), (), (), ()>::builder_with_effects_and_output_events(
            TransitionId::try_new("bounded").unwrap(),
        );
        for _ in 0..16 {
            edge = if effects {
                edge.effect(
                    Effect::builder(EffectId::try_new("effect").unwrap())
                        .describe(|_: &u64, _: &(), _: &()| ())
                        .build()
                        .unwrap(),
                )
                .unwrap()
            } else {
                edge.output_event(
                    OutputEvent::builder(event())
                        .describe(|_: &u64, _: &(), _: &()| ())
                        .build()
                        .unwrap(),
                )
                .unwrap()
            };
        }
        let result = if effects {
            edge.effect(
                Effect::builder(EffectId::try_new("extra").unwrap())
                    .describe(|_: &u64, _: &(), _: &()| ())
                    .build()
                    .unwrap(),
            )
        } else {
            edge.output_event(
                OutputEvent::builder(event())
                    .describe(|_: &u64, _: &(), _: &()| ())
                    .build()
                    .unwrap(),
            )
        };
        assert!(matches!(
            (effects, result),
            (true, Err(TransitionBuildError::EffectLimitExceeded))
                | (false, Err(TransitionBuildError::OutputEventLimitExceeded))
        ));
    }
}
