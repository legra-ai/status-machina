//! Caller-owned policy and serialization contracts for composed machines.
use std::sync::{
    Barrier,
    Mutex,
};
use std::thread;

use status_machina::{
    Capability,
    CapabilityStatus,
    Condition,
    Event,
    LocalAction,
    MachineDefinition,
    MachineIds,
    StateDefinition,
    Transition,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Ids;
impl MachineIds for Ids {
    type Definition = u8;
    type Instance = u8;
    type State = u8;
    type Event = u8;
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
    type Behavior = u8;
    type Implementation = u8;
}
#[derive(Clone, Copy)]
struct Facts {
    available: bool,
    published: bool,
    authority: bool,
    epoch: Option<u64>,
    current: u64,
}
impl Facts {
    fn fresh(self) -> bool {
        self.epoch == Some(self.current)
    }
}
fn definition() -> MachineDefinition<u64, (), Facts, (), (), Ids> {
    MachineDefinition::builder(1)
        .initial(0)
        .state(StateDefinition::builder(0).build())
        .unwrap()
        .transition(
            Transition::builder(1)
                .from(0)
                .on(1)
                .to(0)
                .when(
                    Condition::builder(1)
                        .evaluate(|_: &u64, _: &(), facts: &Facts| facts.fresh() && facts.authority)
                        .build()
                        .unwrap(),
                )
                .action(
                    LocalAction::builder(1)
                        .apply(|data: &mut u64, _: &(), _: &Facts| {
                            *data += 1;
                            Ok(())
                        })
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
fn availability_and_publication_never_replace_fresh_authority() {
    let definition = definition();
    let capability = Capability::builder(1)
        .evaluate(|_: &u8, _: &u64, facts: &Facts| {
            if !facts.fresh() {
                CapabilityStatus::Unknown
            } else if facts.available && facts.published {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Unavailable
            }
        })
        .build()
        .unwrap();
    for flags in 0_u8..8 {
        let available = flags & 1 != 0;
        let published = flags & 2 != 0;
        let authority = flags & 4 != 0;
        for epoch in [None, Some(6), Some(7)] {
            let facts = Facts {
                available,
                published,
                authority,
                epoch,
                current: 7,
            };
            let mut instance = definition.instantiate(1, 0).unwrap();
            let report = instance.evaluate_capability(&capability, &facts).unwrap();
            let expected = if !facts.fresh() {
                CapabilityStatus::Unknown
            } else if available && published {
                CapabilityStatus::Available
            } else {
                CapabilityStatus::Unavailable
            };
            assert_eq!(report.status(), expected);
            let outcome = instance.dispatch(Event::new(1, ()), &facts).unwrap();
            assert_eq!(outcome.is_transitioned(), facts.fresh() && authority);
            assert_eq!(*instance.data(), u64::from(facts.fresh() && authority));
        }
    }
}

#[test]
fn concurrent_callers_serialize_dispatch_without_mutating_another_instance() {
    let definition = definition();
    let shared = Mutex::new(definition.instantiate(1, 0).unwrap());
    let other = definition.instantiate(2, 0).unwrap();
    let barrier = Barrier::new(2);
    let facts = Facts {
        available: true,
        published: true,
        authority: true,
        epoch: Some(7),
        current: 7,
    };
    thread::scope(|scope| {
        // bounded: exactly two caller threads, each dispatching 32 events.
        let callers: [_; 2] = std::array::from_fn(|_| {
            scope.spawn(|| {
                barrier.wait();
                for _ in 0..32 {
                    assert!(
                        shared
                            .lock()
                            .unwrap()
                            .dispatch(Event::new(1, ()), &facts)
                            .unwrap()
                            .is_transitioned()
                    );
                }
            })
        });
        for caller in callers {
            caller.join().unwrap();
        }
    });
    let instance = shared.lock().unwrap();
    assert_eq!(*instance.data(), 64);
    assert_eq!(instance.revision(), 64);
    assert_eq!(*other.data(), 0);
    assert_eq!(other.revision(), 0);
}
