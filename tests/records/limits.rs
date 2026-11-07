use status_machina::{
    ConditionBuildError,
    DispositionBuildError,
    DispositionRecord,
};

use super::*;

#[test]
fn graph_records_preserve_native_limits_and_structural_errors() {
    let registry = registry();
    let states = [
        StateRecord::new(State::Running),
        StateRecord::new(State::Running),
    ];
    let mut record = DefinitionRecord::new(1, State::Running, &states, &[]);
    assert!(matches!(
        record.bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::DuplicateState { .. }
        ))
    ));
    record.max_states = 1;
    assert!(matches!(
        record.bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::StateLimitExceeded { limit: 1 }
        ))
    ));
    record.max_states = 0;
    assert!(matches!(
        record.bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::EmptyLimits
        ))
    ));
    let states = [
        StateRecord::new(State::Running).terminal(),
        StateRecord::new(State::Done),
    ];
    let edges = [TransitionDefinitionRecord::new(
        1,
        State::Running,
        Kind::Complete,
        State::Done,
    )];
    assert!(matches!(
        DefinitionRecord::new(1, State::Running, &states, &edges).bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::TerminalHasOutgoingTransition { .. }
        ))
    ));
    assert!(matches!(
        DefinitionRecord::new(1, State::Running, &states, &[]).bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::UnreachableState { .. }
        ))
    ));
}

#[test]
fn condition_records_reject_empty_wide_and_deep_input() {
    let registry = registry();
    let states = [
        StateRecord::new(State::Running),
        StateRecord::new(State::Done),
    ];
    let check = |condition: &ConditionRecord<'_, Ids>| {
        let mut edge =
            TransitionDefinitionRecord::new(1, State::Running, Kind::Complete, State::Done);
        edge.guard = Some(condition);
        DefinitionRecord::new(1, State::Running, &states, &[edge])
            .bind(&registry)
            .err()
            .unwrap()
    };
    assert!(matches!(
        check(&ConditionRecord::All {
            id: 1,
            operands: &[]
        }),
        RecordBuildError::Condition {
            source: ConditionBuildError::EmptyGroup,
            ..
        }
    ));
    let leaf = || ConditionRecord::Predicate(BehaviorUse::new(1, reference(Story::Allowed)));
    let operands: [_; 17] = std::array::from_fn(|index| {
        ConditionRecord::Predicate(BehaviorUse::new(
            u8::try_from(index).unwrap(),
            reference(Story::Allowed),
        ))
    });
    assert!(matches!(
        check(&ConditionRecord::All {
            id: 20,
            operands: &operands
        }),
        RecordBuildError::Condition {
            source: ConditionBuildError::OperandLimitExceeded,
            ..
        }
    ));
    fn descend<'a>(
        depth: u8,
        parent: &'a ConditionRecord<'a, Ids>,
        check: &impl Fn(&ConditionRecord<'_, Ids>) -> RecordBuildError<Ids>,
    ) {
        if depth == 0 {
            assert!(matches!(
                check(parent),
                RecordBuildError::Condition {
                    source: ConditionBuildError::DepthLimitExceeded,
                    ..
                }
            ));
            return;
        }
        let next = ConditionRecord::Not {
            id: depth,
            operand: parent,
        };
        descend(depth - 1, &next, check);
    }
    descend(32, &leaf(), &check);
}

#[test]
fn dispositions_reject_invalid_explanations_and_duplicate_ids() {
    let registry = registry();
    let states = [StateRecord::new(State::Running)];
    let condition = ConditionRecord::Predicate(BehaviorUse::new(1, reference(Story::Allowed)));
    let disposition = |reason| DispositionRecord {
        id: 1,
        from: State::Running,
        event: Kind::Complete,
        condition: &condition,
        reason,
    };
    let too_long = "x".repeat(257);
    let invalid = [disposition(&too_long)];
    let mut record = DefinitionRecord::new(1, State::Running, &states, &[]);
    record.dispositions = &invalid;
    assert!(matches!(
        record.bind(&registry),
        Err(RecordBuildError::Disposition {
            source: DispositionBuildError::InvalidReason,
            ..
        })
    ));
    let duplicates = [disposition("stale"), disposition("stale")];
    record.dispositions = &duplicates;
    assert!(matches!(
        record.bind(&registry),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::DuplicateDisposition { .. }
        ))
    ));
}
