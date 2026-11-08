# Typed, caller-driven state machines

Status Machina validates immutable machine definitions and advances instances only
when their owner dispatches an event. It creates no threads, clocks, timers, I/O
or persistence. Guards inspect caller-supplied facts; local actions change an
isolated candidate; accepted effects describe work for the caller to execute.

## Start with enum identities

States and events need not be strings. This complete example uses Rust enums and
a caller-supplied numeric instance identity. Diagram labels and serialization are
separate choices; neither changes equality or requires an identifier conversion.

```rust
use status_machina::{Event, MachineDefinition, MachineIds, StateDefinition, Transition};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Ids;
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum State { Pending, Packed }
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Input { Pack }

impl MachineIds for Ids {
    type Definition = u8;
    type Instance = u64;
    type State = State;
    type Event = Input;
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

let definition = MachineDefinition::<(), (), (), (), (), Ids>::builder(1)
    .initial(State::Pending)
    .state(StateDefinition::builder(State::Pending).build())?
    .state(StateDefinition::builder(State::Packed).terminal().build())?
    .transition(Transition::builder(1)
        .from(State::Pending).on(Input::Pack).to(State::Packed).build()?)?
    .build()?;
let mut parcel = definition.instantiate(42, ())?;
assert!(parcel.dispatch(Event::new(Input::Pack, ()), &())?.is_transitioned());
assert_eq!(parcel.state(), &State::Packed);
assert_eq!(parcel.revision(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Unused identifier roles use integers here to keep the introduction short. The
executable tutorials below give definitions, instances, states, events, transitions
and guards distinct domain types. Their [`DiagramLabels`] implementation supplies
readable text without making that text an identity.

## Guarded order: acceptance and external work

The first tutorial adds data, a guard, a local action, a destination invariant,
external work descriptions and an output event. The diagram is generated from
the exact definition that the executable and regression tests use.
