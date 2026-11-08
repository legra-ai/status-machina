//! Native callback failures retain causes and stop each invocation boundary.
use std::fmt;
use std::rc::Rc;

use status_machina::{
    ActionId,
    Capability,
    CapabilityId,
    Condition,
    ConditionId,
    DispatchError,
    EvaluationError,
    EvaluationPhase,
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

#[derive(Debug)]
struct NativeCause(Rc<u8>);
impl fmt::Display for NativeCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native cause {}", self.0)
    }
}
impl std::error::Error for NativeCause {}
fn cause() -> EvaluationError {
    EvaluationError::from_source(NativeCause(Rc::new(7)))
}
fn state(s: &str) -> StateId {
    StateId::try_new(s).unwrap()
}
fn definition(action: LocalAction<u64, (), ()>) -> MachineDefinition<u64, (), ()> {
    MachineDefinition::builder(MachineId::try_new("test").unwrap())
        .initial(state("ready"))
        .state(StateDefinition::builder(state("ready")).build())
        .unwrap()
        .state(StateDefinition::builder(state("done")).build())
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("go").unwrap())
                .from(state("ready"))
                .to(state("done"))
                .on(EventId::try_new("go").unwrap())
                .action(action)
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}
#[test]
fn action_exception_preserves_thread_bound_cause_and_candidate_isolation() {
    let action = LocalAction::builder(ActionId::try_new("fails").unwrap())
        .try_apply(|data: &mut u64, _: &(), _: &()| {
            *data = 99;
            Err(cause())
        })
        .build()
        .unwrap();
    let definition = definition(action);
    let mut instance = definition
        .instantiate(MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(), 7)
        .unwrap();
    let error = instance
        .dispatch(Event::new(EventId::try_new("go").unwrap(), ()), &())
        .unwrap_err();
    let DispatchError::EvaluationFailed { phase, error } = error else {
        panic!("wrong error")
    };
    assert_eq!(phase, EvaluationPhase::Action);
    assert!(error.cause().unwrap().is::<NativeCause>());
    assert_eq!(error, error.clone());
    assert_eq!(instance.state(), &state("ready"));
    assert_eq!(*instance.data(), 7);
    assert_eq!(instance.revision(), 0);
}
#[test]
fn capability_exception_is_not_unknown_or_unavailable() {
    let definition = definition(
        LocalAction::builder(ActionId::try_new("noop").unwrap())
            .apply(|_: &mut u64, _: &(), _: &()| Ok(()))
            .build()
            .unwrap(),
    );
    let instance = definition
        .instantiate(MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(), 7)
        .unwrap();
    let capability = Capability::builder(CapabilityId::try_new("can").unwrap())
        .try_evaluate(|_: &StateId, _: &u64, _: &()| Err(cause()))
        .build()
        .unwrap();
    let error = instance.evaluate_capability(&capability, &()).unwrap_err();
    assert!(error.cause().unwrap().is::<NativeCause>());
    assert_eq!(instance.revision(), 0);
}
#[test]
fn negated_predicate_exception_is_not_negated_into_success() {
    let condition = Condition::<u64, (), ()>::not(ConditionId::try_new("not").unwrap())
        .condition(
            Condition::builder(ConditionId::try_new("fails").unwrap())
                .try_evaluate(|_: &u64, _: &(), _: &()| Err(cause()))
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let definition = MachineDefinition::builder(MachineId::try_new("test").unwrap())
        .initial(state("ready"))
        .state(StateDefinition::builder(state("ready")).build())
        .unwrap()
        .state(StateDefinition::builder(state("done")).build())
        .unwrap()
        .transition(
            Transition::builder(TransitionId::try_new("go").unwrap())
                .from(state("ready"))
                .to(state("done"))
                .on(EventId::try_new("go").unwrap())
                .when(condition)
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut instance = definition
        .instantiate(MachineInstanceId::try_new(uuid::Uuid::now_v7()).unwrap(), 7)
        .unwrap();
    assert!(matches!(
        instance.dispatch(Event::new(EventId::try_new("go").unwrap(), ()), &()),
        Err(DispatchError::EvaluationFailed {
            phase: EvaluationPhase::Guard,
            ..
        })
    ));
    assert_eq!(instance.revision(), 0);
}
