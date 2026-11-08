//! Callback errors stop evaluation before acceptance or subsequent callbacks.
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};

use status_machina::{
    Condition,
    ConditionId,
    DispatchError,
    Effect,
    EffectId,
    EvaluationError,
    EvaluationPhase,
    Event,
    EventId,
    FallibleCandidateData,
    MachineDefinition,
    MachineId,
    MachineInstanceId,
    StateDefinition,
    StateId,
    Transition,
    TransitionId,
};

struct Data {
    value: u64,
    fail_snapshot: bool,
}
impl FallibleCandidateData for Data {
    fn try_snapshot(&self) -> Result<Self, EvaluationError> {
        if self.fail_snapshot {
            return Err(EvaluationError::new("snapshot failed"));
        }
        Ok(Self {
            value: self.value,
            fail_snapshot: false,
        })
    }
}
fn state(value: &str) -> StateId {
    StateId::try_new(value).unwrap()
}
fn event() -> Event<()> {
    Event::new(EventId::try_new("go").unwrap(), ())
}
fn edge() -> status_machina::TransitionBuilder<Data, (), (), u64> {
    Transition::builder_with_effects(TransitionId::try_new("go").unwrap())
        .from(state("ready"))
        .on(event().id().clone())
        .to(state("done"))
}
fn machine(edge: Transition<Data, (), (), u64>) -> MachineDefinition<Data, (), (), u64> {
    MachineDefinition::builder_with_effects(MachineId::try_new("test").unwrap())
        .initial(state("ready"))
        .state(StateDefinition::builder(state("ready")).build())
        .unwrap()
        .state(StateDefinition::builder(state("done")).build())
        .unwrap()
        .transition(edge)
        .unwrap()
        .build()
        .unwrap()
}
fn assert_failure(error: DispatchError, phase: EvaluationPhase, expected: &str) {
    match error {
        DispatchError::EvaluationFailed {
            phase: actual,
            error,
        } => {
            assert_eq!(actual, phase);
            assert_eq!(error.to_string(), expected);
        }
        other => panic!("wrong failure: {other:?}"),
    }
}
#[test]
fn guard_exception_is_not_refusal_and_stops_later_callbacks() {
    let calls = Arc::new(AtomicUsize::new(0));
    let later = calls.clone();
    let guard = Condition::all(ConditionId::try_new("all").unwrap())
        .condition(
            Condition::builder(ConditionId::try_new("fails").unwrap())
                .try_evaluate(|_: &Data, _: &(), _: &()| Err(EvaluationError::new("guard failed")))
                .build()
                .unwrap(),
        )
        .unwrap()
        .condition(
            Condition::builder(ConditionId::try_new("later").unwrap())
                .evaluate(move |_: &Data, _: &(), _: &()| {
                    later.fetch_add(1, Ordering::SeqCst);
                    true
                })
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let definition = machine(edge().when(guard).build().unwrap());
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data {
                value: 7,
                fail_snapshot: false,
            },
        )
        .unwrap();
    assert_failure(
        instance.dispatch(event(), &()).unwrap_err(),
        EvaluationPhase::Guard,
        "guard failed",
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(instance.state(), &state("ready"));
    assert_eq!(instance.data().value, 7);
    assert_eq!(instance.revision(), 0);
}
#[test]
fn snapshot_exception_is_reported_without_committing() {
    let definition = machine(edge().build().unwrap());
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data {
                value: 7,
                fail_snapshot: true,
            },
        )
        .unwrap();
    assert_failure(
        instance.dispatch(event(), &()).unwrap_err(),
        EvaluationPhase::Snapshot,
        "snapshot failed",
    );
    assert_eq!(instance.state(), &state("ready"));
    assert_eq!(instance.revision(), 0);
}
#[test]
fn effect_exception_drops_prior_intents_and_stops_later_constructors() {
    let calls = Arc::new(AtomicUsize::new(0));
    let later = calls.clone();
    let edge = edge()
        .effect(
            Effect::builder(EffectId::try_new("first").unwrap())
                .describe(|_: &Data, _: &(), _: &()| 1)
                .build()
                .unwrap(),
        )
        .unwrap()
        .effect(
            Effect::builder(EffectId::try_new("fails").unwrap())
                .try_describe(|_: &Data, _: &(), _: &()| Err(EvaluationError::new("effect failed")))
                .build()
                .unwrap(),
        )
        .unwrap()
        .effect(
            Effect::builder(EffectId::try_new("later").unwrap())
                .describe(move |_: &Data, _: &(), _: &()| {
                    later.fetch_add(1, Ordering::SeqCst);
                    3
                })
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let definition = machine(edge);
    let mut instance = definition
        .instantiate(
            MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(),
            Data {
                value: 7,
                fail_snapshot: false,
            },
        )
        .unwrap();
    assert_failure(
        instance.dispatch(event(), &()).unwrap_err(),
        EvaluationPhase::Effect,
        "effect failed",
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(instance.state(), &state("ready"));
    assert_eq!(instance.revision(), 0);
}
