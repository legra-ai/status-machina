//! Data-only records reach the same checked builder path as native builders.
use status_machina::{
    BehaviorRef,
    BehaviorRegistry,
    BehaviorUse,
    ConditionRecord,
    DefinitionRecord,
    ForbiddenEdge,
    RecordBuildError,
    StateRecord,
    TransitionDefinitionRecord,
    TransitionKind,
};

use super::{
    Definition,
    DefinitionBuildError,
    EVENT_KINDS,
    Edge,
    Error,
    Ids,
    STATES,
    Session,
    Signal,
    Story,
    forbidden,
    never,
    sealing_cancelled,
    session_builder,
    with_legal_edges,
};

type Registry = BehaviorRegistry<u32, (), (), (), (), Ids>;
type Record<'a> = TransitionDefinitionRecord<'a, Ids>;

fn registry() -> Registry {
    BehaviorRegistry::builder()
        .condition(BehaviorRef::new(Story::Never, 0), |_, _, _| false)
        .unwrap()
        .build()
}

fn state_records() -> Vec<StateRecord<'static, Ids>> {
    STATES
        .into_iter()
        .map(|state| {
            let record = StateRecord::new(state);
            if matches!(state, Session::Done | Session::Failed | Session::Cancelled) {
                record.terminal()
            } else {
                record
            }
        })
        .collect()
}

fn legal_records() -> Vec<Record<'static>> {
    vec![
        Record::new(Edge::Seal, Session::Open, Signal::Seal, Session::Sealing),
        Record::new(
            Edge::Finish,
            Session::Sealing,
            Signal::Finish,
            Session::Done,
        ),
        Record::new(Edge::Fail, Session::Sealing, Signal::Fail, Session::Failed),
        Record::new(
            Edge::Cancel,
            Session::Open,
            Signal::Cancel,
            Session::Cancelled,
        ),
    ]
}

fn bind(
    transitions: &[Record<'_>],
    forbidden_edges: &[ForbiddenEdge<Ids>],
) -> Result<Definition, Error> {
    let states = state_records();
    let mut record = DefinitionRecord::new(1, Session::Open, &states, transitions);
    record.max_states = 16;
    record.max_transitions = 64;
    record.forbidden_edges = forbidden_edges;
    record.bind(&registry()).map_err(|error| match error {
        RecordBuildError::Definition(error) => error,
        other => panic!("unexpected record error {other:?}"),
    })
}

fn native(
    extra: Option<Record<'_>>,
    guarded: bool,
    forbidden_edges: &[ForbiddenEdge<Ids>],
) -> Result<Definition, Error> {
    let mut builder = with_legal_edges(session_builder());
    if let Some(record) = extra {
        let mut transition =
            status_machina::Transition::builder_with_effects_and_output_events(record.id)
                .from(record.from)
                .on(record.event)
                .to(record.to);
        if record.kind == TransitionKind::Internal {
            transition = transition.internal();
        }
        if guarded {
            transition = transition.when(never(1));
        }
        builder = builder.transition(transition.build().unwrap()).unwrap();
    }
    for edge in forbidden_edges {
        builder = builder.forbid(edge.clone()).unwrap();
    }
    builder.build()
}

fn outcome(result: Result<Definition, Error>) -> Result<Vec<ForbiddenEdge<Ids>>, Error> {
    result.map(|definition| definition.forbidden_edges().to_vec())
}

#[test]
fn record_binding_rejects_sealing_to_cancelled_like_the_native_builder() {
    let guard = ConditionRecord::Predicate(BehaviorUse::new(1, BehaviorRef::new(Story::Never, 0)));
    for kind in EVENT_KINDS {
        for guarded in [false, true] {
            let mut conflict = Record::new(
                Edge::SealingCancel,
                Session::Sealing,
                kind,
                Session::Cancelled,
            );
            if guarded {
                conflict.guard = Some(&guard);
            }
            let expected = outcome(native(
                Some(conflict.clone()),
                guarded,
                &[sealing_cancelled()],
            ));
            assert!(matches!(
                expected,
                Err(DefinitionBuildError::ForbiddenTransition {
                    transition: Edge::SealingCancel,
                    ..
                })
            ));
            // Conflicting edge first or last; prohibition first or last.
            let mut first = vec![conflict.clone()];
            first.extend(legal_records());
            let mut last = legal_records();
            last.push(conflict);
            let other = forbidden(Session::Open, Session::Done);
            for transitions in [&first, &last] {
                for prohibitions in [
                    vec![sealing_cancelled(), other.clone()],
                    vec![other.clone(), sealing_cancelled()],
                ] {
                    assert_eq!(
                        outcome(bind(transitions, &prohibitions)),
                        expected,
                        "{kind:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn record_and_native_validation_outcomes_are_identical() {
    let internal = {
        let mut record = Record::new(
            Edge::Retry,
            Session::Sealing,
            Signal::Retry,
            Session::Sealing,
        );
        record.kind = TransitionKind::Internal;
        record
    };
    let external = Record::new(
        Edge::Retry,
        Session::Sealing,
        Signal::Retry,
        Session::Sealing,
    );
    let terminal = Record::new(Edge::Retry, Session::Done, Signal::Retry, Session::Sealing);
    let cases: Vec<(Option<Record<'static>>, Vec<ForbiddenEdge<Ids>>)> = vec![
        (None, vec![sealing_cancelled()]),
        (None, vec![forbidden(Session::Ghost, Session::Cancelled)]),
        (None, vec![forbidden(Session::Sealing, Session::Ghost)]),
        (None, vec![sealing_cancelled(), sealing_cancelled()]),
        (
            Some(internal),
            vec![forbidden(Session::Sealing, Session::Sealing)],
        ),
        (
            Some(external),
            vec![forbidden(Session::Sealing, Session::Sealing)],
        ),
        (
            Some(terminal.clone()),
            vec![forbidden(Session::Done, Session::Sealing)],
        ),
        (Some(terminal), vec![]),
    ];
    for (extra, prohibitions) in cases {
        let mut transitions = legal_records();
        transitions.extend(extra.clone());
        let expected = outcome(native(extra, false, &prohibitions));
        assert_eq!(
            outcome(bind(&transitions, &prohibitions)),
            expected,
            "{prohibitions:?}"
        );
    }
}

#[test]
fn an_oversized_record_inventory_is_rejected_before_any_binding() {
    // An unresolvable Story would fail binding; the constraint bound is
    // checked first, before any state, transition or behavior is bound.
    let unknown =
        ConditionRecord::Predicate(BehaviorUse::new(1, BehaviorRef::new(Story::Always, 0)));
    let mut transitions = legal_records();
    transitions[0].guard = Some(&unknown);
    let prohibitions = vec![sealing_cancelled(); 129];
    let states = state_records();
    let mut record = DefinitionRecord::new(1, Session::Open, &states, &transitions);
    record.forbidden_edges = &prohibitions;
    assert!(matches!(
        record.bind(&registry()),
        Err(RecordBuildError::Definition(
            DefinitionBuildError::ForbiddenEdgeLimitExceeded { limit: 128 }
        ))
    ));
    let accepted = vec![sealing_cancelled(); 128];
    record.forbidden_edges = &accepted;
    assert!(matches!(
        record.bind(&registry()),
        Err(RecordBuildError::Binding(_))
    ));
}

#[test]
fn records_without_prohibitions_keep_binding_unchanged() {
    let definition = bind(&legal_records(), &[]).unwrap();
    assert!(definition.forbidden_edges().is_empty());
}
