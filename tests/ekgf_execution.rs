//! Typed projection of the EKGF execution lifecycle's six states and seven
//! routes. This fixture models the EKGF execution lifecycle structure.
//! This is an executable record example, not an RDF reader or an
//! occurrence-history writer.
use state_machine::{
    BehaviorRef,
    BehaviorRegistry,
    BehaviorUse,
    DefinitionRecord,
    DispatchOutcome,
    Event,
    MachineIds,
    StateRecord,
    TransitionDefinitionRecord,
};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct Execution;
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum State {
    Admitted,
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum EventKind {
    Queue,
    Start,
    Complete,
    Fail,
    Cancel,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Route {
    AdmitQueue,
    QueueRun,
    RunDone,
    RunFail,
    RunCancel,
    AdmitCancel,
    QueueCancel,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Story {
    Finish,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Implementation {
    LocalRules,
}
impl MachineIds for Execution {
    type Definition = u8;
    type Instance = u8;
    type State = State;
    type Event = EventKind;
    type Transition = Route;
    type Action = u8;
    type Condition = u8;
    type Effect = u8;
    type Disposition = u8;
    type Capability = u8;
    type Operation = u8;
    type Scenario = u8;
    type Assertion = u8;
    type Scope = u8;
    type Behavior = Story;
    type Implementation = Implementation;
}

#[test]
fn execution_lifecycle_routes_are_explicit_and_story_bindings_are_independent() {
    let reference = BehaviorRef::new(Story::Finish, Implementation::LocalRules);
    let registry = BehaviorRegistry::<u64, (), (), (), (), Execution>::builder()
        .action(reference.clone(), |data, _, _| {
            *data += 1;
            Ok(())
        })
        .unwrap()
        .build();
    // Intentionally shuffled: declaration/display order cannot imply routes or
    // initial state.
    let states = [
        StateRecord::new(State::Failed).terminal(),
        StateRecord::new(State::Running),
        StateRecord::new(State::Done).terminal(),
        StateRecord::new(State::Admitted),
        StateRecord::new(State::Cancelled).terminal(),
        StateRecord::new(State::Queued),
    ];
    let finish = [BehaviorUse::new(1, reference.clone())];
    let mut done = TransitionDefinitionRecord::new(
        Route::RunDone,
        State::Running,
        EventKind::Complete,
        State::Done,
    );
    done.actions = &finish;
    let routes = [
        TransitionDefinitionRecord::new(
            Route::AdmitQueue,
            State::Admitted,
            EventKind::Queue,
            State::Queued,
        ),
        TransitionDefinitionRecord::new(
            Route::QueueRun,
            State::Queued,
            EventKind::Start,
            State::Running,
        ),
        done,
        TransitionDefinitionRecord::new(
            Route::RunFail,
            State::Running,
            EventKind::Fail,
            State::Failed,
        ),
        TransitionDefinitionRecord::new(
            Route::RunCancel,
            State::Running,
            EventKind::Cancel,
            State::Cancelled,
        ),
        TransitionDefinitionRecord::new(
            Route::AdmitCancel,
            State::Admitted,
            EventKind::Cancel,
            State::Cancelled,
        ),
        TransitionDefinitionRecord::new(
            Route::QueueCancel,
            State::Queued,
            EventKind::Cancel,
            State::Cancelled,
        ),
    ];
    let definition = DefinitionRecord::new(1, State::Admitted, &states, &routes)
        .bind(&registry)
        .unwrap();
    assert!(definition.bindings().eq([&reference]));
    let paths: [(&[EventKind], State); 5] = [
        (
            &[EventKind::Queue, EventKind::Start, EventKind::Complete],
            State::Done,
        ),
        (
            &[EventKind::Queue, EventKind::Start, EventKind::Fail],
            State::Failed,
        ),
        (
            &[EventKind::Queue, EventKind::Start, EventKind::Cancel],
            State::Cancelled,
        ),
        (&[EventKind::Cancel], State::Cancelled),
        (&[EventKind::Queue, EventKind::Cancel], State::Cancelled),
    ];
    for (events, end) in paths {
        let mut instance = definition.instantiate(1, 0).unwrap();
        assert_eq!(instance.state(), &State::Admitted);
        assert!(
            instance
                .dispatch(Event::new(EventKind::Complete, ()), &())
                .unwrap()
                .is_refused()
        );
        for (index, event) in events.iter().enumerate() {
            let DispatchOutcome::Transitioned { record, .. } = instance
                .dispatch(Event::new(event.clone(), ()), &())
                .unwrap()
            else {
                panic!("permitted route refused")
            };
            assert_eq!(record.previous_revision(), u64::try_from(index).unwrap());
            assert_eq!(record.revision(), u64::try_from(index + 1).unwrap());
        }
        assert_eq!(instance.state(), &end);
        assert_eq!(*instance.data(), u64::from(end == State::Done));
        assert!(
            instance
                .dispatch(Event::new(EventKind::Queue, ()), &())
                .unwrap()
                .is_refused()
        );
    }
}
