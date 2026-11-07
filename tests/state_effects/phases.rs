use super::*;

#[derive(Debug, Eq, PartialEq)]
struct Request {
    phase: u8,
    candidate: u64,
}

fn action(name: &str, increment: u64) -> LocalAction<u64, (), ()> {
    LocalAction::builder(ActionId::try_new(name).unwrap())
        .apply(move |data: &mut u64, _: &(), _: &()| {
            *data += increment;
            Ok(())
        })
        .build()
        .unwrap()
}

#[test]
fn distinct_state_effects_observe_final_candidate_and_acceptance_runs_no_callbacks() {
    let calls = Arc::new(AtomicUsize::new(0));
    let described = |phase| {
        let calls = Arc::clone(&calls);
        Effect::builder(EffectId::try_new(format!("phase-{phase}")).unwrap())
            .describe(move |data: &u64, _: &(), _: &()| {
                calls.fetch_add(1, Ordering::SeqCst);
                Request {
                    phase,
                    candidate: *data,
                }
            })
            .build()
            .unwrap()
    };
    let destination = StateId::try_new("done").unwrap();
    let definition = MachineDefinition::<u64, (), (), Request>::builder_with_effects(
        MachineId::try_new("phases").unwrap(),
    )
    .initial(state())
    .state(
        StateDefinition::builder(state())
            .exit_action(action("exit", 1))
            .unwrap()
            .exit_effect(described(1))
            .unwrap()
            .entry_effect(described(99))
            .unwrap()
            .build(),
    )
    .unwrap()
    .state(
        StateDefinition::builder(destination.clone())
            .terminal()
            .invariant(|data| *data == 111)
            .entry_action(action("entry", 100))
            .unwrap()
            .entry_effect(described(3))
            .unwrap()
            .exit_effect(described(98))
            .unwrap()
            .build(),
    )
    .unwrap()
    .transition(
        Transition::builder_with_effects(TransitionId::try_new("advance").unwrap())
            .from(state())
            .on(event().id().clone())
            .to(destination.clone())
            .action(action("transition", 10))
            .unwrap()
            .effect(described(2))
            .unwrap()
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition.instantiate(identity(), 0).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let decision = instance.evaluate(event(), &()).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(*instance.data(), 0);
    let DispatchOutcome::Transitioned { effects, .. } = instance.accept(decision).unwrap() else {
        panic!("expected transition")
    };
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(*instance.data(), 111);
    assert_eq!(instance.state(), &destination);
    assert_eq!(effects.len(), 3);
    for (effect, phase) in effects.into_iter().zip([1, 2, 3]) {
        assert_eq!(
            effect.into_parts().1,
            Request {
                phase,
                candidate: 111
            }
        );
    }
}

struct OwnedRequest(Arc<AtomicUsize>);
impl Drop for OwnedRequest {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn discarding_or_rejecting_decisions_releases_owned_payloads_without_acceptance() {
    let dropped = Arc::new(AtomicUsize::new(0));
    let captured = Arc::clone(&dropped);
    let definition = MachineDefinition::<u64, (), (), OwnedRequest>::builder_with_effects(
        MachineId::try_new("drop").unwrap(),
    )
    .initial(state())
    .state(
        StateDefinition::builder(state())
            .entry_effect(
                Effect::builder(EffectId::try_new("owned").unwrap())
                    .describe(move |_: &u64, _: &(), _: &()| OwnedRequest(Arc::clone(&captured)))
                    .build()
                    .unwrap(),
            )
            .unwrap()
            .build(),
    )
    .unwrap()
    .transition(
        Transition::builder_with_effects(TransitionId::try_new("tick").unwrap())
            .from(state())
            .on(event().id().clone())
            .to(state())
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    let mut instance = definition.instantiate(identity(), 7).unwrap();
    drop(instance.evaluate(event(), &()).unwrap());
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    let first = instance.evaluate(event(), &()).unwrap();
    let stale = instance.evaluate(event(), &()).unwrap();
    let accepted = instance.accept(first).unwrap();
    assert!(matches!(
        instance.accept(stale),
        Err(DispatchError::StaleRevision { .. })
    ));
    assert_eq!(dropped.load(Ordering::SeqCst), 2);
    assert_eq!(instance.revision(), 1);
    assert_eq!(*instance.data(), 7);
    drop(accepted);
    assert_eq!(dropped.load(Ordering::SeqCst), 3);
}
