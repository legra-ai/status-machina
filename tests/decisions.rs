//! Evaluation produces an owned decision; only checked acceptance changes an
//! instance.
use state_machine::{
    ActionId,
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

#[derive(Debug, Eq, PartialEq)]
struct Request(u64);
fn state() -> StateId {
    StateId::try_new("active").unwrap()
}
fn event() -> Event<()> {
    Event::new(EventId::try_new("tick").unwrap(), ())
}
fn identity(number: u128) -> MachineInstanceId {
    MachineInstanceId::try_new(uuid::Uuid::from_u128(
        0x0000_0000_0000_7000_8000_0000_0000_0000 + number,
    ))
    .unwrap()
}
fn definition(name: &str) -> MachineDefinition<u64, (), (), Request> {
    MachineDefinition::builder_with_effects(MachineId::try_new(name).unwrap())
        .initial(state())
        .state(StateDefinition::builder(state()).build())
        .unwrap()
        .transition(
            Transition::builder_with_effects(TransitionId::try_new("tick").unwrap())
                .from(state())
                .on(event().id().clone())
                .to(state())
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
                .effect(
                    Effect::builder(EffectId::try_new("request").unwrap())
                        .describe(|data: &u64, _: &(), _: &()| Request(*data))
                        .build()
                        .unwrap(),
                )
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn evaluation_and_dropping_decisions_leave_accepted_data_unchanged() {
    let definition = definition("counter");
    let mut instance = definition.instantiate(identity(1), 0).unwrap();
    let discarded = instance.evaluate(event(), &()).unwrap();
    assert_eq!(*instance.data(), 0);
    assert_eq!(instance.revision(), 0);
    drop(discarded);
    let decision = instance.evaluate(event(), &()).unwrap();
    let DispatchOutcome::Transitioned {
        effects, record, ..
    } = instance.accept(decision).unwrap()
    else {
        panic!("expected acceptance")
    };
    assert_eq!(
        effects.into_iter().next().unwrap().into_parts().1,
        Request(1)
    );
    assert_eq!(record.revision(), 1);
    assert_eq!(*instance.data(), 1);
}

#[test]
fn competing_decision_is_stale_after_first_acceptance() {
    let definition = definition("counter");
    let mut instance = definition.instantiate(identity(1), 0).unwrap();
    let first = instance.evaluate(event(), &()).unwrap();
    let second = instance.evaluate(event(), &()).unwrap();
    instance.accept(first).unwrap();
    assert!(matches!(
        instance.accept(second),
        Err(DispatchError::StaleRevision {
            expected: 0,
            current: 1
        })
    ));
    assert_eq!(*instance.data(), 1);
    assert_eq!(instance.revision(), 1);
}

#[test]
fn acceptance_rejects_other_instance_and_definition_identities() {
    let first_definition = definition("first");
    let other_definition = definition("other");
    let source = first_definition.instantiate(identity(1), 0).unwrap();
    let mut other_instance = first_definition.instantiate(identity(2), 7).unwrap();
    let mut other_machine = other_definition.instantiate(identity(1), 9).unwrap();
    assert!(matches!(
        other_instance.accept(source.evaluate(event(), &()).unwrap()),
        Err(DispatchError::InstanceMismatch { .. })
    ));
    assert!(matches!(
        other_machine.accept(source.evaluate(event(), &()).unwrap()),
        Err(DispatchError::DefinitionMismatch { .. })
    ));
    assert_eq!(*other_instance.data(), 7);
    assert_eq!(*other_machine.data(), 9);
    assert_eq!(other_instance.revision(), 0);
    assert_eq!(other_machine.revision(), 0);
}

#[test]
fn external_acceptor_validates_persisted_identity_and_revision_before_consuming() {
    let definition = definition("counter");
    let source = definition.instantiate(identity(1), 4).unwrap();
    let decision = source.evaluate(event(), &()).unwrap();
    assert_eq!(decision.definition_id(), definition.id());
    assert_eq!(decision.instance_id(), &identity(1));
    assert_eq!(decision.expected_revision(), 0);
    let validated = decision
        .validate_for(definition.id(), &identity(1), 0)
        .unwrap();
    let (candidate, outcome) = validated.into_parts();
    assert_eq!(candidate, Some(5));
    assert!(outcome.is_transitioned());
    assert_eq!(*source.data(), 4);
    assert_eq!(source.revision(), 0);
    assert!(matches!(
        source
            .evaluate(event(), &())
            .unwrap()
            .validate_for(definition.id(), &identity(1), 1),
        Err(DispatchError::StaleRevision { .. })
    ));
}

#[test]
fn refused_decisions_do_not_increment_revision_and_are_still_identity_checked() {
    let definition = definition("counter");
    let mut instance = definition.instantiate(identity(1), 4).unwrap();
    let decision = instance
        .evaluate(Event::new(EventId::try_new("unknown").unwrap(), ()), &())
        .unwrap();
    assert!(instance.accept(decision).unwrap().is_refused());
    assert_eq!(*instance.data(), 4);
    assert_eq!(instance.revision(), 0);
}
