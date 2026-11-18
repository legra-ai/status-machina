//! Restoring an instance at a declared state: the state must exist and the
//! restored data must satisfy that state's invariant; a restored instance
//! dispatches exactly like one that walked there.

use std::sync::Arc;

use status_machina::{
    Event,
    MachineDefinition,
    MachineIds,
    RestoreError,
    StateDefinition,
    Transition,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Ids;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum State {
    Pending,
    Open,
    Closed,
    /// Never declared in the definition.
    Undeclared,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Open,
    Close,
}

impl MachineIds for Ids {
    type Definition = u16;
    type Instance = u64;
    type State = State;
    type Event = Kind;
    type Transition = u8;
    type Action = u8;
    type Condition = u8;
    type Effect = u8;
    type Disposition = u8;
    type Capability = u8;
    type Operation = u8;
    type Scenario = u8;
    type Assertion = u8;
    type Scope = u8;
    type Behavior = u16;
    type Implementation = u16;
}

/// The number of times the instance was opened.
type Opens = u32;
type Definition = MachineDefinition<Opens, (), (), (), (), Ids>;

fn edge(id: u8, from: State, on: Kind, to: State) -> Transition<Opens, (), (), (), (), Ids> {
    Transition::builder(id)
        .from(from)
        .on(on)
        .to(to)
        .build()
        .unwrap()
}

fn definition() -> Definition {
    Definition::builder(1)
        .initial(State::Pending)
        .state(
            StateDefinition::builder(State::Pending)
                .invariant(|opens: &Opens| *opens == 0)
                .build(),
        )
        .unwrap()
        .state(StateDefinition::builder(State::Open).build())
        .unwrap()
        .state(StateDefinition::builder(State::Closed).terminal().build())
        .unwrap()
        .transition(edge(1, State::Pending, Kind::Open, State::Open))
        .unwrap()
        .transition(edge(2, State::Open, Kind::Close, State::Closed))
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn restoring_at_every_declared_state_is_accepted() {
    let definition = definition();
    for state in [State::Pending, State::Open, State::Closed] {
        let instance = definition.restore(7, state, 0).unwrap();
        assert_eq!(instance.state(), &state);
        assert_eq!(instance.data(), &0);
        assert_eq!(instance.revision(), 0);
        assert_eq!(instance.instance_id(), 7);
    }
}

#[test]
fn restoring_at_an_undeclared_state_is_refused() {
    let definition = definition();
    let Err(error) = definition.restore(7, State::Undeclared, 0) else {
        panic!("an undeclared state must be refused");
    };
    assert_eq!(
        error,
        RestoreError::UnknownState {
            state: State::Undeclared
        }
    );
}

#[test]
fn restoring_data_that_violates_the_state_invariant_is_refused() {
    let definition = definition();
    let Err(error) = definition.restore(7, State::Pending, 3) else {
        panic!("data violating the pending invariant must be refused");
    };
    assert_eq!(
        error,
        RestoreError::InvariantFailed {
            state: State::Pending
        }
    );
}

#[test]
fn a_restored_instance_dispatches_normally() {
    let definition = definition();
    let mut open = definition.restore(7, State::Open, 1).unwrap();
    assert!(
        open.dispatch(Event::new(Kind::Open, ()), &())
            .unwrap()
            .is_refused()
    );
    assert!(
        open.dispatch(Event::new(Kind::Close, ()), &())
            .unwrap()
            .is_transitioned()
    );
    assert_eq!(open.state(), &State::Closed);
    assert_eq!(open.revision(), 1);

    let mut closed = definition.restore(7, State::Closed, 1).unwrap();
    assert!(
        closed
            .dispatch(Event::new(Kind::Close, ()), &())
            .unwrap()
            .is_refused()
    );
}

#[test]
fn a_shared_restore_outlives_the_callers_handle() {
    let definition = Arc::new(definition());
    let mut instance = Arc::clone(&definition)
        .restore_shared(7, State::Open, 1)
        .unwrap();
    drop(definition);
    assert!(
        instance
            .dispatch(Event::new(Kind::Close, ()), &())
            .unwrap()
            .is_transitioned()
    );
    let Err(error) = Arc::new(self::definition()).restore_shared(7, State::Undeclared, 0) else {
        panic!("an undeclared state must be refused");
    };
    assert_eq!(
        error,
        RestoreError::UnknownState {
            state: State::Undeclared
        }
    );
}
