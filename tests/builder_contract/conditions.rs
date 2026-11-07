use status_machina::{
    ActionBuildError,
    ActionId,
    ConditionBuildError,
    Effect,
    EffectBuildError,
    EffectId,
    LocalAction,
    OutputEvent,
    OutputEventBuildError,
};

use super::*;

#[test]
fn missing_callback_implementations_are_rejected_during_construction() {
    assert!(matches!(
        Condition::<u64, (), ()>::builder(ConditionId::try_new("missing").unwrap()).build(),
        Err(ConditionBuildError::MissingPredicate)
    ));
    assert!(matches!(
        LocalAction::<u64, (), ()>::builder(ActionId::try_new("missing").unwrap()).build(),
        Err(ActionBuildError::MissingImplementation)
    ));
    assert!(matches!(
        Effect::<u64, (), (), ()>::builder(EffectId::try_new("missing").unwrap()).build(),
        Err(EffectBuildError::MissingConstructor)
    ));
    assert!(matches!(
        OutputEvent::<u64, (), (), ()>::builder(EventId::try_new("missing").unwrap()).build(),
        Err(OutputEventBuildError::MissingConstructor)
    ));
}

#[test]
fn condition_builders_reject_empty_and_repeated_operands() {
    let id = || ConditionId::try_new("group").unwrap();
    assert!(matches!(
        Condition::<u64, (), ()>::all(id()).build(),
        Err(ConditionBuildError::EmptyGroup)
    ));
    assert!(matches!(
        Condition::<u64, (), ()>::any(id()).build(),
        Err(ConditionBuildError::EmptyGroup)
    ));
    assert!(matches!(
        Condition::<u64, (), ()>::not(id()).build(),
        Err(ConditionBuildError::EmptyNegation)
    ));
    assert!(matches!(
        Condition::not(id())
            .condition(predicate())
            .unwrap()
            .condition(predicate()),
        Err(ConditionBuildError::DuplicateOperand)
    ));
}

#[test]
fn conditions_enforce_operand_and_depth_boundaries() {
    for all in [true, false] {
        let id = ConditionId::try_new("group").unwrap();
        let mut group = if all {
            Condition::all(id)
        } else {
            Condition::any(id)
        };
        for _ in 0..16 {
            group = group.condition(predicate()).unwrap();
        }
        assert!(matches!(
            group.condition(predicate()),
            Err(ConditionBuildError::OperandLimitExceeded)
        ));
    }
    let nested = || {
        let mut condition = predicate();
        for _ in 1..32 {
            condition = Condition::not(ConditionId::try_new("nested").unwrap())
                .condition(condition)
                .unwrap()
                .build()
                .unwrap();
        }
        condition
    };
    assert!(matches!(
        Condition::not(ConditionId::try_new("too-deep").unwrap()).condition(nested()),
        Err(ConditionBuildError::DepthLimitExceeded)
    ));
    assert!(matches!(
        Condition::all(ConditionId::try_new("too-deep").unwrap()).condition(nested()),
        Err(ConditionBuildError::DepthLimitExceeded)
    ));
    assert!(matches!(
        Condition::any(ConditionId::try_new("too-deep").unwrap()).condition(nested()),
        Err(ConditionBuildError::DepthLimitExceeded)
    ));
}
