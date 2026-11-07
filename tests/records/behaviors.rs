use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use status_machina::{
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
            status_machina::BehaviorRef::new(
                status_machina::BehaviorId::new(1),
                status_machina::ImplementationId::new(1),
            ),
            move |data, _, _| {
                action_calls.fetch_add(1, Ordering::SeqCst);
                *data += 1;
                Ok(())
            },
        )
        .unwrap()
        .condition(
            status_machina::BehaviorRef::new(
                status_machina::BehaviorId::new(2),
                status_machina::ImplementationId::new(1),
            ),
            move |_, _, _| {
                condition_calls.fetch_add(1, Ordering::SeqCst);
                true
            },
        )
        .unwrap()
        .invariant(
            status_machina::BehaviorRef::new(
                status_machina::BehaviorId::new(3),
                status_machina::ImplementationId::new(1),
            ),
            move |data| {
                invariant_calls.fetch_add(1, Ordering::SeqCst);
                *data <= 3
            },
        )
        .unwrap()
        .effect(
            status_machina::BehaviorRef::new(
                status_machina::BehaviorId::new(4),
                status_machina::ImplementationId::new(1),
            ),
            move |data, _, _| {
                effect_calls.fetch_add(1, Ordering::SeqCst);
                *data
            },
        )
        .unwrap()
        .output_event(
            status_machina::BehaviorRef::new(
                status_machina::BehaviorId::new(5),
                status_machina::ImplementationId::new(1),
            ),
            move |data, _, _| {
                output_calls.fetch_add(1, Ordering::SeqCst);
                *data
            },
        )
        .unwrap()
        .build();
    let reference = |id| {
        status_machina::BehaviorRef::new(
            status_machina::BehaviorId::new(id),
            status_machina::ImplementationId::new(1),
        )
    };
    let action = [BehaviorUse::new(
        status_machina::ActionId::try_new("increment").unwrap(),
        reference(1),
    )];
    let effect = [BehaviorUse::new(
        status_machina::EffectId::try_new("notify").unwrap(),
        reference(4),
    )];
    let output = [BehaviorUse::new(
        status_machina::EventId::try_new("output").unwrap(),
        reference(5),
    )];
    let leaf = ConditionRecord::Predicate(BehaviorUse::new(
        status_machina::ConditionId::try_new("allowed").unwrap(),
        reference(2),
    ));
    let postconditions = [ConditionRecord::All {
        id: status_machina::ConditionId::try_new("all").unwrap(),
        operands: std::slice::from_ref(&leaf),
    }];
    let running = status_machina::StateId::try_new("running").unwrap();
    let done = status_machina::StateId::try_new("done").unwrap();
    let complete = status_machina::EventId::try_new("complete").unwrap();
    let mut source = StateRecord::new(running.clone());
    source.exit_actions = &action;
    source.exit_effects = &effect;
    let mut destination = StateRecord::new(done.clone()).terminal();
    destination.invariant = Some(reference(3));
    destination.entry_actions = &action;
    destination.entry_effects = &effect;
    let states = [source, destination];
    let mut edge = TransitionDefinitionRecord::new(
        status_machina::TransitionId::try_new("finish").unwrap(),
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
    let ignored_event = status_machina::EventId::try_new("stale").unwrap();
    let dispositions = [DispositionRecord {
        id: status_machina::DispositionId::try_new("ignore").unwrap(),
        from: done,
        event: ignored_event.clone(),
        condition: &leaf,
        reason: "stale completion",
    }];
    let mut records = DefinitionRecord::new(
        status_machina::MachineId::try_new("execution").unwrap(),
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
            status_machina::MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
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
