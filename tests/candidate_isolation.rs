//! Candidate mutations must not escape a rejected transition.

use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use status_machina::{
    ActionId,
    CandidateData,
    Condition,
    ConditionId,
    DispatchError,
    Effect,
    EffectId,
    Event,
    EventId,
    LocalAction,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    OutputEvent,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

struct Data {
    mutable: Arc<AtomicUsize>,
    immutable: Arc<str>,
    invalid: bool,
}

impl Data {
    fn new() -> Self {
        Self {
            mutable: Arc::new(AtomicUsize::new(0)),
            immutable: Arc::from("shared immutable configuration"),
            invalid: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    ActionFailure,
    PostconditionFailure,
    InvariantFailure,
    Accept,
}

fn definition(intents: &Arc<AtomicUsize>) -> MachineDefinition<Data, Outcome, (), (), ()> {
    let idle = StateId::try_new("idle").unwrap();
    let complete = StateId::try_new("complete").unwrap();
    let effect_calls = Arc::clone(intents);
    let output_calls = Arc::clone(intents);
    let transition = Transition::builder_with_effects_and_output_events(
        TransitionId::try_new("change").unwrap(),
    )
    .from(idle.clone())
    .on(EventId::try_new("change").unwrap())
    .to(complete.clone())
    .action(
        LocalAction::builder(ActionId::try_new("mutate-candidate").unwrap())
            .apply(|data: &mut Data, event: &Outcome, _: &()| {
                data.mutable.fetch_add(1, Ordering::SeqCst);
                data.invalid = matches!(event, Outcome::InvariantFailure);
                if matches!(event, Outcome::ActionFailure) {
                    return Err("reject candidate".to_owned());
                }
                Ok(())
            })
            .build()
            .unwrap(),
    )
    .unwrap()
    .postcondition(
        Condition::builder(ConditionId::try_new("accept").unwrap())
            .evaluate(|_: &Data, event: &Outcome, _: &()| {
                !matches!(event, Outcome::PostconditionFailure)
            })
            .build()
            .unwrap(),
    )
    .unwrap()
    .effect(
        Effect::builder(EffectId::try_new("effect").unwrap())
            .describe(move |_: &Data, _: &Outcome, _: &()| {
                effect_calls.fetch_add(1, Ordering::SeqCst);
            })
            .build()
            .unwrap(),
    )
    .unwrap()
    .output_event(
        OutputEvent::builder(EventId::try_new("output").unwrap())
            .describe(move |_: &Data, _: &Outcome, _: &()| {
                output_calls.fetch_add(1, Ordering::SeqCst);
            })
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    MachineDefinition::builder_with_effects_and_output_events(
        MachineId::try_new("isolation").unwrap(),
    )
    .initial(idle.clone())
    .state(StateDefinition::builder(idle).build())
    .unwrap()
    .state(
        StateDefinition::builder(complete)
            .invariant(|data: &Data| !data.invalid)
            .build(),
    )
    .unwrap()
    .transition(transition)
    .unwrap()
    .build()
    .unwrap()
}

#[test]
fn rejected_candidates_preserve_accepted_data_and_release_no_intents() {
    for outcome in [
        Outcome::ActionFailure,
        Outcome::PostconditionFailure,
        Outcome::InvariantFailure,
    ] {
        let intents = Arc::new(AtomicUsize::new(0));
        let definition = definition(&intents);
        let data = Data::new();
        let accepted_mutable = Arc::clone(&data.mutable);
        let identity = MachineInstanceId::try_new(uuid::Uuid::from_u128(
            0x0000_0000_0000_7000_8000_0000_0000_0001,
        ))
        .unwrap();
        let mut instance = definition.instantiate(identity, data).unwrap();
        let error = instance
            .dispatch(
                Event::new(EventId::try_new("change").unwrap(), outcome),
                &(),
            )
            .unwrap_err();
        assert!(matches!(
            (outcome, error),
            (Outcome::ActionFailure, DispatchError::ActionFailed { .. })
                | (
                    Outcome::PostconditionFailure,
                    DispatchError::PostconditionFailed { .. }
                )
                | (
                    Outcome::InvariantFailure,
                    DispatchError::InvariantFailed { .. }
                )
        ));
        assert_eq!(instance.data().mutable.load(Ordering::SeqCst), 0);
        assert_eq!(accepted_mutable.load(Ordering::SeqCst), 0);
        assert!(Arc::ptr_eq(&accepted_mutable, &instance.data().mutable));
        assert_eq!(instance.state().as_str(), "idle");
        assert_eq!(instance.revision(), 0);
        assert_eq!(intents.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn accepted_candidate_replaces_mutable_storage_but_can_share_immutable_data() {
    let intents = Arc::new(AtomicUsize::new(0));
    let definition = definition(&intents);
    let data = Data::new();
    let old_mutable = Arc::clone(&data.mutable);
    let immutable = Arc::clone(&data.immutable);
    let identity = MachineInstanceId::try_new(uuid::Uuid::from_u128(
        0x0000_0000_0000_7000_8000_0000_0000_0002,
    ))
    .unwrap();
    let mut instance = definition.instantiate(identity, data).unwrap();
    assert!(
        instance
            .dispatch(
                Event::new(EventId::try_new("change").unwrap(), Outcome::Accept),
                &()
            )
            .unwrap()
            .is_transitioned()
    );
    assert_eq!(old_mutable.load(Ordering::SeqCst), 0);
    assert_eq!(instance.data().mutable.load(Ordering::SeqCst), 1);
    assert!(!Arc::ptr_eq(&old_mutable, &instance.data().mutable));
    assert!(Arc::ptr_eq(&immutable, &instance.data().immutable));
    assert_eq!(instance.state().as_str(), "complete");
    assert_eq!(instance.revision(), 1);
    assert_eq!(intents.load(Ordering::SeqCst), 2);
}

impl CandidateData for Data {
    fn snapshot(&self) -> Self {
        Self {
            mutable: Arc::new(AtomicUsize::new(self.mutable.load(Ordering::SeqCst))),
            immutable: Arc::clone(&self.immutable),
            invalid: self.invalid,
        }
    }
}
