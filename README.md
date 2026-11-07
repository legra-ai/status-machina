# Status Machina

A typed, caller-driven extended finite-state machine for Rust. Definitions are
immutable; instances advance only when their owner dispatches an event.

The core supplies guards, local actions, invariants, postconditions, entry/exit
behavior, typed effect intents and output events. Explicit bounded composition,
capability observations, retry policies, diagrams and finite scenario replay
support inspection and coordination. Callers own execution, time, I/O and
persistence; the crate starts no tasks, threads or timers.

## Example

Each definition selects a `MachineIds` family. States and event kinds can be
Rust enums; instance identity can be a caller-owned newtype. The core never
converts them to strings or generates IDs.

```rust
use status_machina::{Event, MachineDefinition, MachineIds, StateDefinition, Transition};
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct OrderIds;
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum State { Pending, Complete }
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Kind { Finish }
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Instance(u64);
impl MachineIds for OrderIds {
    type Definition = u16;
    type Instance = Instance;
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
let transition = Transition::<(), (), (), (), (), OrderIds>::builder(1)
    .from(State::Pending).on(Kind::Finish).to(State::Complete)
    .build().unwrap();
let definition = MachineDefinition::<(), (), (), (), (), OrderIds>::builder(42)
    .initial(State::Pending)
    .state(StateDefinition::builder(State::Pending).build()).unwrap()
    .state(StateDefinition::builder(State::Complete).build()).unwrap()
    .transition(transition).unwrap().build().unwrap();
let mut instance = definition.instantiate(Instance(7), ()).unwrap();
let decision = instance.evaluate(Event::new(Kind::Finish, ()), &()).unwrap();
assert_eq!(instance.revision(), 0); // Evaluation has not changed the instance.
assert!(instance.accept(decision).unwrap().is_transitioned());
assert_eq!(instance.state(), &State::Complete);
```

`MachineDefinition<D, E, F, X, O, I>` separates local data, event payloads,
caller-supplied facts, effect payloads, output-event payloads and the identity
family. Event kind and payload remain separate. Identity values need
`Clone + Debug + Eq + Hash`; dispatch needs neither `Display`, `Ord` nor Serde.
Use distinct types for roles that must not be interchangeable. The family marker
also derives these traits for generic value records. Values and identity operations
must have caller-bounded size and cost and stable equality/hashing.

`DefaultIds` offers the validated string wrappers and caller-supplied UUIDv7 IDs
as one convenience policy. Standalone builders may need an explicit family:
associated types alone cannot uniquely identify their implementing family.

`Composition<E, SourceIds, DestinationIds>::builder(sources, destinations)` takes
separate bounded instance registries. Routes explicitly name the source event and
destination event, and construction checks both declared event inventories and
payload signatures. Capability observations use the destination family. The
caller retains ownership of routed payloads and performs destination dispatch.

Inspection adds ordering bounds only where canonical ordering is needed.
`diagram_with_labels` takes a `DiagramLabels` provider; duplicate labels do not
merge states. `IdentityMap::try_new(entries, max_entries)` separately validates a
bounded bijection between typed IDs and external values. Its `encode`/`decode`
methods reject unknown values; construction rejects collisions. This is an adapter
primitive, not a serializer for executable definitions or closures.

## Evaluation and acceptance

`evaluate` (or `evaluate_at`) creates an owned `MachineDecision` without changing
accepted state, data or revision. The decision seals its candidate and intent
payloads. `accept` consumes it, checks definition identity, instance identity and
revision, then installs the candidate and returns the intents. Competing proposals
become stale after the first transition is accepted. Dropping a proposal discards
its owned data and payloads. `dispatch` and `dispatch_at` combine these same steps.

For an external durable acceptor, `MachineDecision::validate_for` exposes a
`ValidatedDecision` only after checking supplied identities and revision.
`into_parts` transfers optional candidate data and the outcome without cloning.
The adapter must read the current identity/revision and persist candidate state
and intents within one atomic transaction, then execute intents after commit.
Validation is not a storage commit; this crate implements no persistence adapter.
Acceptance runs no callbacks and does not refresh externally supplied facts.
The owner determines whether those facts remain admissible and must keep each
definition ID bound to one immutable executable definition and each instance ID
unique to its incarnation.

States may declare `entry_effect` and `exit_effect`, using
`StateDefinition<D, E, F, X, I>`. External transitions prepare effects in declared
exit → transition → entry order after all actions, postconditions and destination
invariants pass. Every effect constructor sees the final candidate. Internal
self-transitions prepare only transition effects. Initialization prepares none.
Each phase permits 16 effects, giving at most 48 per external transition; output
events remain bounded at 16 per transition. Constructors describe work; the caller
executes it only after acceptance.

## Current boundaries

- This is a flat EFSM, without hierarchical states or orthogonal regions.
- Data implements `CandidateData::snapshot` to produce an equivalent, isolated
  candidate. There is no blanket `Clone`/`Copy` implementation: cloning shared
  mutable references does not provide rollback. Scalar values and `()` have
  built-in implementations; domain types explicitly implement the contract.
  Shared immutable storage is allowed. Callbacks must be deterministic, bounded
  and free of hidden side effects; only local actions may mutate the candidate.
  Rust cannot prove snapshot isolation or callback purity. Panics and external
  effects are not transactional recovery mechanisms.
- `EffectIntent::into_parts` and `OutputEventIntent::into_parts` transfer the
  identifier and payload to caller-owned execution. `RoutedBatch::into_items`
  transfers ordered routing outcomes so events can move directly into dispatch.
  None of these operations requires `Clone` on the payload.
- Graph and work counts have bounds; generic payload byte sizes, snapshot costs
  and callback runtimes remain the caller's responsibility. Scenario replay
  checks supplied finite traces, not every possible execution.
- Definitions contain executable Rust callbacks. There is no portable
  definition serialization, RDF/DSL reader, or durable instance restoration.
- The direct dependencies are `thiserror` and `uuid`.

## Design terminology

Named behavior references correspond to **Stories** in the EKGF
Story Ontology (`story:Story`), with a separately identified
`story:StoryImplementation`. A reference carries typed Story and implementation
identities; enums/newtypes can represent both. A graph element's identity marks
its use site, while the Story identity identifies reusable behavior. Names and
RDF IRIs are explicit external representations, not the kernel's identity model.
The generic core retains typed binding and pure local evaluation; callers execute
Stories requiring external I/O through accepted effect intents.


`MachineIds::Behavior` and `MachineIds::Implementation` select these independent
identity types. For example, an application can use `enum Story { ValidateOrder,
SubmitOrder }` and `enum Implementation { Rules2026 }`; neither requires `Display`
or Serde. The convenience family uses distinct numeric `BehaviorId` and
`ImplementationId` newtypes.

`BehaviorRegistry::builder()` registers trusted implementations of actions,
conditions, invariants, effects, output events, capabilities and scenario
assertions. `BehaviorRef::new(story, implementation)` selects an exact pair;
`resolve_action(use_site_id, &reference)` (and the other role-specific resolvers)
returns a behavior accepted by the existing builders. Rust types enforce payload
signatures, and resolution rejects unknown Stories, unknown implementations and
wrong callback roles without executing callbacks. Multiple implementations of
one Story must have the same role; duplicate pair declarations are errors.
Registration defaults to 128 declarations, configurable up to 4096.

Resolved behaviors retain their reference and own their callback, so the registry
can be dropped after construction. `MachineDefinition::bindings()` inspects
references at state, transition and disposition use sites, including nested
conditions and repeated uses. Capabilities and scenario assertions expose their
own `binding()`. Native inline closures remain supported and have no registered
reference; inspection does not certify that a definition is portable. Registrants
are responsible for assigning a different implementation identity when behavior
changes: identity equality cannot prove code equivalence.

`DefinitionRecord` provides a data-only graph view containing `StateRecord`,
`TransitionDefinitionRecord`, `ConditionRecord` and `DispositionRecord` values.
Behavior use sites carry `BehaviorUse` references. `record.bind(&registry)`
resolves them through the registry and constructs the graph through the same
validated builders used by native Rust callers. It executes no callbacks and
returns an owned definition independent of the borrowed records and registry.
Required endpoints/events/initial state are typed fields, not inferred labels.
Records borrow slices; binding retains only the executable graph under native
limits. Format readers must bound their own source storage and identity sizes.
Serialization readers remain separate work; this API does not load code, resolve
RDF, or execute a Story service.

## EKGF Life Cycle Ontology

The EKGF Lifecycle Ontology supplies the structural vocabulary around these
Stories. An adapter can map `lifecycle:Lifecycle` to a definition,
`lifecycle:State` to a state, and `lifecycle:Transition` to a permitted edge
(`fromState`, `toState`, `inLifecycle`). `isTerminal` corresponds to the native
terminal-state rule. The compiled `tests/ekgf_execution.rs` example projects the
well-known execution lifecycle's six states and seven routes into enum-identified
records and exercises completion, failure and cancellation with a Story binding.

`lifecycle:trigger` is human-readable description, not an event identity, guard
or authorization rule. An executable projection must explicitly supply typed
events, the initial state and any Story implementations. Phases and `order` are
presentation metadata; order does not create transitions or select the initial
state. Membership does not require a separate RDF named graph.

`lifecycle:StateTransition` represents an actual state-entry occurrence, distinct
from a permitted transition definition. A caller-owned history adapter can
project accepted external transitions using `currentState` and
`priorStateTransition`, and record initial entry separately. Native internal
self-transitions do not enter a state. The kernel's `TransitionRecord` is an
acceptance result, not an RDF occurrence identifier or a durable history chain.
Lifecycle assignment (`followsLifecycle`/`appliesTo`), provenance, occurrence
identity and persistence stay with the adapter. No lifecycle or RDF dependency
is added to the generic crate.

## Generated diagram examples

[View generated Mermaid diagrams on GitHub](docs/diagrams.md): the EKGF execution
lifecycle and a retry/completion example with guards and dispositions. The page includes a regeneration command, and a
regression test keeps its Mermaid blocks synchronized with the definitions.

## License

Licensed under either MIT or Apache-2.0, at your option.

## Synchronous composition

Validated connections can also be bound to exclusive mutable instance references
with `Composition::bind`. Dispatch then propagates connected events synchronously
within explicit work limits. Reports retain deferred or interrupted work and
caller-owned effects. Release bindings and caller locks before executing external
effects. This adds no background executor, persistence or clocks.
