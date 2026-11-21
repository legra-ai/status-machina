//! Prohibitions are structural only: dispatch keeps its existing semantics.
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use status_machina::{
    DispatchOutcome,
    Event,
    EventDisposition,
    LocalAction,
};

use super::{
    Definition,
    DefinitionBuildError,
    Edge,
    Session,
    Signal,
    Transition,
    never,
    sealing_cancelled,
    session_builder,
    with_legal_edges,
};

fn legal() -> Definition {
    with_legal_edges(session_builder())
        .forbid(sealing_cancelled())
        .unwrap()
        .disposition(
            EventDisposition::builder(1)
                .from(Session::Sealing)
                .on(Signal::Stale)
                .when(
                    status_machina::Condition::builder(2)
                        .evaluate(|_: &u32, _: &(), _: &()| true)
                        .build()
                        .unwrap(),
                )
                .reason("stale completion")
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn legal_completion_and_failure_edges_remain_usable() {
    let definition = legal();
    for (signal, target) in [
        (Signal::Finish, Session::Done),
        (Signal::Fail, Session::Failed),
    ] {
        let mut instance = definition.instantiate(1, 0).unwrap();
        assert!(
            instance
                .dispatch(Event::new(Signal::Seal, ()), &())
                .unwrap()
                .is_transitioned()
        );
        assert!(
            instance
                .dispatch(Event::new(signal, ()), &())
                .unwrap()
                .is_transitioned()
        );
        assert_eq!(instance.state(), &target);
        assert_eq!(instance.revision(), 2);
    }
}

#[test]
fn the_forbidden_event_still_refuses_without_mutation_or_intents() {
    let definition = legal();
    let mut instance = definition.instantiate(1, 7).unwrap();
    instance
        .dispatch(Event::new(Signal::Seal, ()), &())
        .unwrap();
    let outcome = instance
        .dispatch(Event::new(Signal::Cancel, ()), &())
        .unwrap();
    assert!(matches!(
        outcome,
        DispatchOutcome::Refused {
            state: Session::Sealing,
            event: Signal::Cancel
        }
    ));
    assert_eq!(instance.state(), &Session::Sealing);
    assert_eq!(instance.revision(), 1);
    assert_eq!(instance.data(), &7);
}

#[test]
fn explicit_dispositions_keep_their_documented_behavior() {
    let definition = legal();
    let mut instance = definition.instantiate(1, 7).unwrap();
    instance
        .dispatch(Event::new(Signal::Seal, ()), &())
        .unwrap();
    let outcome = instance
        .dispatch(Event::new(Signal::Stale, ()), &())
        .unwrap();
    assert!(matches!(
        outcome,
        DispatchOutcome::Ignored { disposition: 1, state: Session::Sealing, event: Signal::Stale, ref reason }
            if reason == "stale completion"
    ));
    assert_eq!(instance.revision(), 1);
    assert_eq!(instance.data(), &7);
}

#[test]
fn construction_rejects_a_forbidden_edge_without_running_any_callback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let guard_calls = Arc::clone(&calls);
    let action_calls = Arc::clone(&calls);
    let transition = Transition::builder_with_effects_and_output_events(Edge::SealingCancel)
        .from(Session::Sealing)
        .on(Signal::Cancel)
        .to(Session::Cancelled)
        .when(
            status_machina::Condition::builder(3)
                .evaluate(move |_: &u32, _: &(), _: &()| {
                    guard_calls.fetch_add(1, Ordering::SeqCst);
                    true
                })
                .build()
                .unwrap(),
        )
        .action(
            LocalAction::builder(1)
                .apply(move |_: &mut u32, _: &(), _: &()| {
                    action_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .build()
                .unwrap(),
        )
        .unwrap()
        .postcondition(never(4))
        .unwrap()
        .build()
        .unwrap();
    let result = with_legal_edges(session_builder())
        .forbid(sealing_cancelled())
        .unwrap()
        .transition(transition)
        .unwrap()
        .build();
    assert!(matches!(
        result,
        Err(DefinitionBuildError::ForbiddenTransition {
            transition: Edge::SealingCancel,
            ..
        })
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
