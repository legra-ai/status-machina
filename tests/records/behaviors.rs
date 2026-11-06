use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use state_machine::{
    DispatchOutcome,
    DispositionRecord,
};

use super::*;

#[test]
fn record_binding_is_callback_free_and_preserves_all_execution_phases() {
    // Exercise the convenience identity family as well as the enum fixtures.
    let calls = Arc::new(AtomicUsize::new(0));
    let action_calls = Arc::clone(&calls);
    let condition_calls = Arc::clone(&calls);
    let invariant_calls = Arc::clone(&calls);
    let effect_calls = Arc::clone(&calls);
    let output_calls = Arc::clone(&calls);
    let registry = BehaviorRegistry::<u64, (), (), u64, u64>::builder()
        .action(
            state_machine::BehaviorRef::new(
                state_machine::BehaviorId::new(1),
                state_machine::ImplementationId::new(1),
            ),
            move |data, _, _| {
                action_calls.fetch_add(1, Ordering::SeqCst);
                *data += 1;
                Ok(())
            },
        )
        .unwrap()
        .condition(
            state_machine::BehaviorRef::new(
                state_machine::BehaviorId::new(2),
                state_machine::ImplementationId::new(1),
            ),
            move |_, _, _| {
                condition_calls.fetch_add(1, Ordering::SeqCst);
                true
            },
        )
        .unwrap()
        .invariant(
            state_machine::BehaviorRef::new(
                state_machine::BehaviorId::new(3),
                state_machine::ImplementationId::new(1),
            ),
            move |data| {
                invariant_calls.fetch_add(1, Ordering::SeqCst);
                *data <= 3
            },
        )
        .unwrap()
        .effect(
            state_machine::BehaviorRef::new(
                state_machine::BehaviorId::new(4),
                state_machine::ImplementationId::new(1),
            ),
            move |data, _, _| {
                effect_calls.fetch_add(1, Ordering::SeqCst);
                *data
            },
        )
        .unwrap()
        .output_event(
            state_machine::BehaviorRef::new(
                state_machine::BehaviorId::new(5),
                state_machine::ImplementationId::new(1),
            ),
            move |data, _, _| {
                output_calls.fetch_add(1, Ordering::SeqCst);
                *data
            },
        )
        .unwrap()
        .build();
    let reference = |id| {
        state_machine::BehaviorRef::new(
            state_machine::BehaviorId::new(id),
            state_machine::ImplementationId::new(1),
        )
    };
    let action = [BehaviorUse::new(
        state_machine::ActionId::try_new("increment").unwrap(),
        reference(1),
    )];
    let effect = [BehaviorUse::new(
        state_machine::EffectId::try_new("notify").unwrap(),
        reference(4),
    )];
    let output = [BehaviorUse::new(
        state_machine::EventId::try_new("output").unwrap(),
        reference(5),
    )];
    let leaf = ConditionRecord::Predicate(BehaviorUse::new(
        state_machine::ConditionId::try_new("allowed").unwrap(),
        reference(2),
    ));
    let postconditions = [ConditionRecord::All {
        id: state_machine::ConditionId::try_new("all").unwrap(),
        operands: std::slice::from_ref(&leaf),
    }];
    let running = state_machine::StateId::try_new("running").unwrap();
    let done = state_machine::StateId::try_new("done").unwrap();
    let complete = state_machine::EventId::try_new("complete").unwrap();
    let mut source = StateRecord::new(running.clone());
    source.exit_actions = &action;
    source.exit_effects = &effect;
    let mut destination = StateRecord::new(done.clone()).terminal();
    destination.invariant = Some(reference(3));
    destination.entry_actions = &action;
    destination.entry_effects = &effect;
    let states = [source, destination];
    let mut edge = TransitionDefinitionRecord::new(
        state_machine::TransitionId::try_new("finish").unwrap(),
        running.clone(),
        complete.clone(),
        done.clone(),
    );
    edge.guard = Some(&leaf);
    edge.postconditions = &postconditions;
    edge.actions = &action;
    edge.effects = &effect;
    edge.output_events = &output;
    let edges = [edge];
    let ignored_event = state_machine::EventId::try_new("stale").unwrap();
    let dispositions = [DispositionRecord {
        id: state_machine::DispositionId::try_new("ignore").unwrap(),
        from: done,
        event: ignored_event.clone(),
        condition: &leaf,
        reason: "stale completion",
    }];
    let mut records = DefinitionRecord::new(
        state_machine::MachineId::try_new("execution").unwrap(),
        running,
        &states,
        &edges,
    );
    records.dispositions = &dispositions;
    let definition = records.bind(&registry).unwrap();
    assert_eq!(definition.bindings().count(), 11);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(registry);
    let mut instance = definition
        .instantiate(
            state_machine::MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            0,
        )
        .unwrap();
    let DispatchOutcome::Transitioned {
        effects, outputs, ..
    } = instance.dispatch(Event::new(complete, ()), &()).unwrap()
    else {
        panic!("expected transition")
    };
    assert_eq!(*instance.data(), 3);
    assert_eq!(effects.len(), 3);
    assert!(effects.iter().all(|effect| *effect.payload() == 3));
    assert_eq!(outputs.len(), 1);
    assert_eq!(*outputs[0].payload(), 3);
    assert!(matches!(
        instance
            .dispatch(Event::new(ignored_event, ()), &())
            .unwrap(),
        DispatchOutcome::Ignored { .. }
    ));
}
