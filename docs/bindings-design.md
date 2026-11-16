# Native Python and Node.js APIs

Status: implementation plan. The Rust core is published; native language bindings
are under implementation and neither package is published yet. This document defines the acceptance
boundary for both packages. A completed design is not a completed binding.

The Python distribution is `status-machina`, imported as `status_machina`.
The npm package is `@legra-ai/status-machina`. Both live in this repository,
use MIT OR Apache-2.0, and consume the same Rust transition kernel. The unscoped
npm name is occupied. Availability checks do not reserve either unpublished name.

## Deliverables

| ID | Deliverable | Depends on | Status |
| --- | --- | --- | --- |
| BND-01 | Native API, identity, payload, callback and ownership contracts, with a complete public-model inventory and release acceptance matrix | — | Done |
| BND-02 | Fallible kernel evaluation and snapshots; callback failure stops evaluation before acceptance, without panic transport or sentinel results | BND-01 | Done |
| BND-03 | Python checked construction, native values, execution and inspection through PyO3, with typed Python source and exceptions | BND-02 | Done |
| BND-04 | Node.js checked construction, native values, execution and inspection through napi-rs, with JavaScript and TypeScript APIs | BND-02 | Partial |
| BND-05 | Native Story bindings, capabilities, bounded composition, retries and scenario coverage, with model-by-model parity evidence in both languages | BND-03, BND-04 | Done |
| BND-06 | Installable wheels, source distribution and npm artifacts; complete examples and installed-artifact tests without a Rust toolchain | BND-05 | Not started |
| BND-07 | Extend the shared tested-revision pipeline to gate and publish all three ecosystems, with synchronized versions and interrupted-publication recovery | BND-06 | Partial |
| BND-08 | Initial native packages published and verified from the approved tested revision on every declared platform | BND-07 | Not started |

The kernel stays independent of Python, Node.js, their dependency graphs and
execution environments. Existing Rust consumers can remain pinned to 0.1.0;
this work does not change their manifests or coordinate their runtime policy.
Breaking Rust API changes are recorded accurately. No major release is authorized
by this plan; a release planner proposing one must stop at its approval gate.

## Native domain values

The native layer owns the language experience. It must not expose Rust generic
parameters, `Result`, Rust enums encoded as arbitrary dictionaries, or opaque
handles in places where the application needs a domain value. Machine and
decision objects may own hidden native resources behind ordinary language APIs.

Python uses user-defined `Enum` members and immutable hashable newtypes for
identifiers; frozen dataclasses for specifications, records and outcomes;
`typing.Generic`/protocols for application payloads; and an exception hierarchy.
Do not require enum values to be strings or JSON-compatible. Dataclass payloads
remain instances of the application's class after snapshots and callbacks.

JavaScript uses ordinary objects and native values. TypeScript exports generic
specification types, discriminated unions and declarations, without Rust handles
in their fields. Identifiers may be symbols, enum values, primitives or stable
object tokens. Object tokens compare by identity, not by stringification or
incidental object layout. TypeScript types distinguish the state, event and other
roles; erased numeric enum types cannot provide an additional runtime brand.
Runtime validation still checks each reference in its declared role's inventory.

Intern identifiers separately for each definition and role. A compact native
index is private to that definition and is never an application identifier or a
portable wire ID. Return the original native value when inspecting a record or
outcome. Never use `str`, `repr`, `toString`, JSON, a display label, or an enum's
underlying number as a universal identity codec. Python keys include their
concrete type so different `IntEnum` classes do not silently alias. Explicit
serialization codecs remain separate from native identity.

Definition, instance, state, event, transition, action, condition, effect,
disposition, capability, operation, scenario, assertion and routing-scope roles
all retain their identity boundaries. Story and implementation identities are
separate native values too. A behavior is what the EKGF Story Ontology calls a
Story; selecting a Story never implicitly selects an implementation. The EKGF
Life Cycle Ontology can describe lifecycle meaning without imposing a particular
identifier representation or changing execution semantics.

## Application-facing shape

Construction uses native specifications passed to one checked definition
constructor. Fluent helpers may improve ergonomics but must use that same
validation path. Unknown states/events, duplicate IDs, malformed conditions,
wrong Story implementations and limit violations fail during construction.

The following illustrates the intended Python API, not an available import:

```python
from dataclasses import dataclass
from enum import Enum, auto
from status_machina import Action, Definition, Event, State, Transition

class Phase(Enum):
    OPEN = auto()
    CLOSED = auto()

class Input(Enum):
    CLOSE = auto()

@dataclass
class Counter:
    closed: int = 0

def close(data: Counter, event: None, facts: None) -> None:
    data.closed += 1

definition = Definition(
    id=object(),
    initial=Phase.OPEN,
    states=(State(Phase.OPEN), State(Phase.CLOSED, terminal=True)),
    transitions=(Transition(
        id=object(), source=Phase.OPEN, event=Input.CLOSE,
        target=Phase.CLOSED, actions=(Action(id=object(), apply=close),),
    ),),
)
with definition.instantiate(id=object(), data=Counter()) as machine:
    outcome = machine.dispatch(Event(Input.CLOSE, None), facts=None)
    assert outcome.kind == "transitioned"
    assert machine.state is Phase.CLOSED
    assert machine.data.closed == 1
```

For Node.js the same contract uses `new Definition({...})`, `instantiate({...})`,
`dispatch({ kind, payload }, facts)`, a `kind`-discriminated result, `state`/`data`
accessors, and explicit `dispose()` / `Symbol.dispose`. TypeScript infers the
identifier and payload types from the specification; the two languages need not
copy each other's constructor syntax. These examples do not settle every helper
name; implementation tests must exercise the documented installed API.

## Data isolation and callback failures

The binding must own accepted data independently of the input and of values
returned by inspection. Snapshot initial data on admission and return detached
snapshots from the data accessor. Never hand accepted mutable data directly to a
callback, including guards, invariants and intent constructors. Actions may mutate
only an isolated candidate. A value retained by a callback must not later become
an alias to accepted mutable data; isolate again before acceptance as needed.

Python's default snapshot operation uses `copy.deepcopy`, preserving native
classes and enums. A caller may supply a pure snapshot function for domain types
with different copy requirements. Node's default copies checked native object
graphs, preserving symbols, cycles, shared references, Buffer and typed views.
It rejects unsupported values and custom class instances with an explicit error. A caller-supplied snapshot function
supports such domain objects explicitly. Custom snapshot functions have the
same non-aliasing, deterministic and bounded-cost obligations as Rust
`CandidateData`; the binding cannot prove purity of arbitrary application code.
Do not claim to enforce a byte budget for arbitrary native object graphs.

Snapshot failure is a typed error, including at instantiation and inspection.
Every native callback stage can fail: guards, actions, postconditions, state
invariants, snapshot functions, effect constructors, output constructors and
routing/capability predicates. The first failure stops evaluation immediately;
no later callback runs, no state/data/revision is accepted, and no intents escape.
Return the native exception as the cause of a stage-specific binding exception.
Keep predicate `False` distinct from a predicate that raised an exception.
Reject coroutine/Promise returns during synchronous evaluation; do not coerce
them to truthy values or schedule them silently.

The published 0.1.0 kernel has infallible snapshots, bool predicates/invariants
and infallible intent constructors. The implementation adds explicit fallible
callbacks and a `FallibleCandidateData` capability, also satisfied by ordinary
Rust `CandidateData`. Errors retain their original native causes and final
candidate isolation happens before decision creation. Regression tests cover
exception propagation, short-circuiting, rollback and thread-bound causes. Do not emulate
failure through a false predicate, empty intent, thread-local error slot, caught
panic or a global panic hook. Native wrappers must not duplicate transition
selection/acceptance in Python or JavaScript to avoid changing the kernel.

Preserve the Rust evaluate/accept boundary. A native decision is single-use and
bound to definition, instance incarnation and expected revision. Acceptance runs
no user callbacks. Snapshot/alias separation needed for acceptance must therefore
finish during evaluation. Discarding a decision releases its candidate/intents.
Stale or foreign decisions fail before mutation. Refused, ignored and transitioned
outcomes remain distinct native variants with typed records and payloads.

## Execution, lifetime and ownership

All evaluation is synchronous on the invoking language thread. Python uses the
interpreter attachment/GIL rules required by the selected PyO3 version. Node
callbacks execute only in the owning Node environment and thread, including
worker-thread environments. Never declare thread-bound callback handles
`Send`/`Sync` through an unsafe implementation. Audit the core's current captured
closure bounds; use safe invocation ownership or an explicit core capability
rather than smuggling Node handles through them.

Reject recursive dispatch, acceptance, disposal or composition mutation while
an operation is active. Serialize operations per instance explicitly; do not
block waiting for a lock held by a callback on the same thread. Independent
instances remain independently usable. Disposed objects fail immediately with a
native disposed-resource error. Disposal is idempotent and releases definitions,
callback references, payloads, pending decisions and routing ownership when no
remaining owner needs them. Definition ownership outlives its instances.

Python supports context managers. Node supports `dispose()` and `Symbol.dispose`;
finalization is only a leak-safety mechanism, never a correctness requirement.
No background executor, event loop, clock, timer, I/O or persistence is introduced.
Effects are values returned after acceptance, executed by the application.

## Public-model parity inventory

Every row needs native documentation, type declarations and positive/negative
tests in each language before BND-05 is complete. Builders are construction
helpers, not additional domain objects that must mechanically be exported.
A row is Covered once both languages have that evidence. Every row is Covered.

| Rust model family | Native counterpart and required evidence | Parity |
| --- | --- | --- |
| `MachineIds`, `Identity`, `IdentityMap`, all identifier roles | Native role-aware identity inventories/codecs; enum/newtype/token round trips, no label-based equality | Covered: [identity codecs](#portable-records-and-identity-codecs) |
| `MachineDefinition`, `DefinitionBuilder`, state/transition builders and kinds | Checked immutable specifications; initial state, terminal/internal/external rules, reachability and limits | Covered: [model certification](#model-certification) |
| `Condition`, expression kinds and builders | Native predicate and all/any/not expressions; bounded depth/operands, short-circuit and exception tests | Covered: [model certification](#model-certification) |
| `LocalAction`, `Effect`, `OutputEvent` | Typed native callbacks and returned intent values, phase order, owned payloads and exception rollback | Covered: [model certification](#model-certification) |
| `BehaviorRef`, registry, roles and `BoundInvariant` | Exact native Story plus implementation references; wrong role/missing implementation rejection | Covered: [model certification](#model-certification) |
| `DefinitionRecord`, state/transition/condition/disposition records, `BehaviorUse` | Native portable specification records and checked binding; no implicit code loading or string IDs | Covered: [portable records](#portable-records-and-identity-codecs) |
| `MachineInstance`, `Event`, `MachineDecision`, `ValidatedDecision` | Native instance/event/single-use decision APIs; owned snapshots, explicit revision checks and disposal | Covered: [model certification](#model-certification) |
| `DispatchOutcome`, `TransitionRecord`, effect/output intents | Native discriminated outcomes and records retaining exact native identifiers and payload types | Covered: [model certification](#model-certification) |
| `EventDisposition` | Explicit harmless-event dispositions, conditions and reasons distinct from refusal | Covered: [model certification](#model-certification) |
| `Capability`, status/report/snapshot | Advisory native capability values and bounded checked snapshots; no hidden authorization authority | Covered: [model certification](#model-certification) |
| `Composition`, routes, registry/addresses, bound dispatch, batches/reports/limits | Explicit typed routing under caller control, multi-machine examples, ordered partial progress on failure, bounded propagation and namespace isolation | Covered: [native composition](#native-composition) |
| `RetryPolicy`, schedule, waits, correlation and wake intent | Native durations/correlations and caller-owned wake values; stale/foreign/cancelled wake tests; no scheduling | Covered: [native retry](#native-retry-and-waits) |
| Scenario sets/replay/assertions/reports/traces/limits | Native replay and independent safety assertions; bounded counterexamples and coverage evidence | Covered: [native scenario replay](#native-scenario-replay) |
| Inspection limits/coverage and diagram labels | Native inspection and Mermaid generation with application labels separate from identity; deterministic escaping and limit errors | Covered: [model certification](#model-certification) |
| All construction, binding, routing, dispatch, retry, scenario and inspection errors | Native exception taxonomy with typed context and original callback causes; no raw Rust `Result` or generic string-only errors | Covered: [model certification](#model-certification) |
| `EventOnDrop` (0.1.1) | Caller-owned scope-exit event: one synchronous delivery while armed, explicit `disarm`, correlation update, no rearming, no delivery by garbage collection | Covered: [model certification](#model-certification) |

Composition is not a distributed transaction: earlier accepted steps remain
accepted if a later delivery fails or hits a propagation bound. Reports must
preserve that progress and unconsumed work as defined by the kernel. Callback
failure still prevents acceptance of the failing step. Documentation and tests
must make this distinction visible.

### Identity codec API

Rust `IdentityMap<T, R>` is a bounded one-to-one map for a single role, and
`MachineIds` is the family of sixteen roles. Neither language can name a Rust
type per role, so the native API makes the role a value and keeps the kernel as
the authority:

- `IdentityRole` lists the `MachineIds` roles. `Behavior` is spelled `story`,
  matching `BehaviorRef.story`: a Python `Enum` and a frozen TypeScript object
  of string literals.
- `IdentityMap` takes a role, `(identity, representation)` entries and
  `max_entries`/`maxEntries` (default and ceiling 4096). Python makes it
  `Generic[IdT, ReprT]`. In TypeScript it is `IdentityMap<R, T, X>`, where the
  role literal `R` is part of the type.
- Entries are interned with the language's own identity rules. Python uses
  type-qualified keys; JavaScript uses `Map` semantics, so object tokens compare
  by identity. The interned index pairs are then validated by the kernel's
  `IdentityMap::try_new` through the bridge (`status_machina_bridge::codec`),
  and every `encode`/`decode` also goes through it. The kernel decides the
  bound, the one-to-one rule and their error precedence. Python and JavaScript
  only bound how many entries they read (at most the bound plus one) and
  resolve the result back to the original native values.
- `MachineIdentityMaps` holds at most one codec per role and rejects one whose
  role differs from its slot. Python uses a frozen dataclass with one field per
  role, generic over the state, event and representation types. TypeScript
  checks the role-keyed object at compile time, and types `encode`/`decode` by
  the configured role.
- Errors are native classes under `IdentityMapError`. The kernel's
  `IdentityMapError` variants become `IdentityMapLimitError`,
  `DuplicateIdentityError`, `DuplicateRepresentationError`,
  `UnknownIdentityError` and `UnknownRepresentationError`. A wrong or missing
  role is an `IdentityRoleError`. Their context holds the role, the limit, the
  entry position and the native identity or representation concerned. In Python
  the unknown-value errors are also `LookupError`s and the limit error is also a
  `LimitError`.

Codecs never feed runtime matching. Adapters decode external names, build
native records or specifications, and encode outcomes afterwards.

## Packaging and release acceptance

Use dotenvage's PyO3/maturin and napi-rs package layout as implementation
references, retaining Status Machina's own license and current shared release
policy. Its older per-registry publisher scripts are not the release template.
Keep binding dependencies in unpublished workspace members, outside the core's
runtime dependency graph. Ship Python `py.typed`, typed source/stubs and native
exceptions; ship JavaScript plus TypeScript declarations and explicit exports.
Neither ordinary package installation may need Rust, Cargo, maturin or napi CLI.

Initial declared support follows a tested matrix: Python 3.10–3.14 and Node.js
22 (from 22.10)/24; Linux glibc x86_64/aarch64, macOS 26 ARM64, and Windows
x86_64/aarch64. Each advertised artifact needs build and installed-package test
evidence on its matching target. The sole macOS CI target is `macos-latest`
(currently macOS 26 ARM64); Intel macOS is outside the native package matrix.
Add other interpreters, musl or architectures
only with equivalent evidence. Unsupported targets fail clearly; no source-build
fallback disguises an absent native artifact. Python source builds are an
explicit developer path tested through the source distribution.

Build wheels and npm native artifacts from the exact shared preparation revision.
Install them in clean environments without repository source paths or a Rust
toolchain. Run full examples, native type checks and contract tests against the
installed packages. Include licenses, documentation and required native modules;
validate source-distribution and npm-tarball contents. An import-only smoke test
is insufficient. Test worker isolation, callback retention/disposal, exception
causes and snapshot rejection as well as successful transitions.

Extend `legra-ai/github-actions` so native checks/artifacts gate the existing sole
publisher, including the Rust release. Synchronize the core, binding manifests,
Python distribution and npm package versions through version preparation hooks.
All required registries and draft-release completion use that tested revision.
Resume interrupted multi-registry publication at its original immutable tag and
verify already-published artifact identity. Do not add a second publisher or
allow a successful Rust-only release to hide failed native artifacts.

Before BND-08 publication, configure and verify the selected PyPI/npm publisher
identities and repository/environment trust. Missing credentials/trust are
visible publication blockers, not permission to skip a registry. Complete all
independent implementation and artifact tests before requesting any owner-only
setup. BND-08 is Done only after downloaded registry packages match the approved
revision and execute the documented examples on the declared support matrix.

## Implementation evidence

BND-02 has passing kernel regressions and the full Rust suite, with pinned
formatter/Clippy hook validation. The private adapter also tests bounded value
leases, checked graph construction, unaccepted decisions, disposal and host-specific
thread capability. Its dependencies do not enter the core runtime graph.

BND-03 is Done and BND-04 is Partial; see [model certification](#model-certification)
for the final evidence. The first lifecycle slice had the compiled Python extension pass 60 native
application and lifecycle tests and the Node addon 61. These cover enum/symbol
identities, domain data, detached snapshots, callback exceptions, checked decision
acceptance, disposal, bounded value-slot reuse and reentrancy. Python also tests
cross-thread rejection during evaluation and garbage collection of callback
cycles. Node tests both CommonJS and ESM entry points and cyclic typed-view data.

Node worker isolation is tested in `npm/tests/workers.test.mjs`. Four workers
and the main thread each build and run their own machines concurrently. Every
callback runs only on the thread that created its machine. A worker's callback
failure keeps its worker-local exception class as the cause. `Definition`,
`Machine` and `Decision` are marked uncloneable, so `postMessage` and `workerData`
throw `DataCloneError` for them, even nested. Before this they silently cloned to
empty objects. The handles stay usable after the rejected send. Workers are
terminated while they hold machines and pending decisions, both idle and inside
a non-returning callback. Termination neither crashes nor hangs, and repeated
rounds stay within a fixed resident-memory bound. `markAsUncloneable` sets the
package floor at Node.js 22.10, and the loader refuses older releases. V8 never
shares objects between workers, so JavaScript cannot reach the napi adapter's
own environment/thread checks from another thread. They remain defensive.

CI builds both modules from source and runs these suites on every pull request.
The `python-bindings` and `node-bindings` jobs cover Ubuntu 24.04, `macos-latest`
and Windows, with Python 3.10/3.14 and Node.js 22/24. They run the bridge, PyO3
and napi crate tests, `maturin develop`, mypy, `tsc --noEmit` and the typing
fixtures. The required `test` gate fails unless every one of them passes. They
test the checkout, not installed artifacts.

BND-07 is Partial: native checks now gate the sole publisher. The shared
`cargo-maintenance.yml` call (`ci`) declares `needs: [prepare, python-bindings,
node-bindings]` without `always()`. Its version commit and its crates.io
publication therefore start only after every binding job, on every runner and
interpreter, succeeded for the run's revision. A failed, cancelled or skipped
job skips the whole release path, so a Rust-only release can no longer hide a
binding failure. `scripts/test_maintenance.py` checks that the release call
depends on every build job of the workflow, and that every shared action and
workflow uses one pinned revision. That pin includes legra-ai/github-actions#114:
the source run only writes the version commit, after its tests and binding
jobs passed, and publication happens only in the version commit's own run,
gated by that run's binding jobs. The published revision is therefore the
revision the bindings tested. Native artifacts, PyPI/npm publication, synchronized versions and
multi-registry recovery remain outstanding.

Python and TypeScript static examples check state/event identity, initial data
and data unions that functional actions can replace. They include deliberately
invalid calls which must remain type errors. Python source passes mypy.

Native capabilities include exact Story bindings, advisory reports tied to the
observed native instance/revision, detached data/facts and original exception
causes. Unknown and duplicate capability identifiers fail explicitly.

Bounded native structural coverage and Mermaid generation call the Rust
inspection APIs. Explicit label maps retain native identities, including distinct
states with equal labels, and fail on missing labels or UTF-8/entry/output limits.
Inspection never evaluates application behavior. Python and Node runnable order
examples generate identical Mermaid embedded in their READMEs; three additional
private Rust tests reject malformed label metadata before renderer lookups.

### Portable records and identity codecs

The native records mirror `DefinitionRecord`, `StateRecord`,
`TransitionDefinitionRecord`, `DispositionRecord`, `ConditionRecord` and
`BehaviorUse`. Python uses frozen dataclasses and TypeScript uses frozen classes.
Condition records are a `kind`-discriminated union (`PredicateRecord`,
`AllRecord`, `AnyRecord`, `NotRecord`). The field names follow the native
specification types (`source`/`target`/`internal`/`outputs`); Rust uses
`from`/`to`/`kind`/`output_events`. A record holds only native identities and
exact `BehaviorRef` selections. Constructors reject callables, plain objects,
foreign values and unknown fields, and bound every phase to 16 entries.

`bind(registry)` lowers the record to the private checked transport with
reference bindings only. `Template::bind_record` rebuilds the borrowed Rust
records in an arena and calls `DefinitionRecord::bind`. It rejects any inline
callback or capability, so no code can be selected except from the explicit
registry. Native recursion stops at the kernel depth of 32, reporting the same
condition identity the kernel would. `RecordBuildError` becomes a native
exception: `BindingError` subclasses for an unknown Story, an unknown
implementation or a role mismatch, record-specific errors naming the
condition/transition/disposition, and `RecordError` for graph rules. Context is
resolved back to the native identities.

Evidence: `bindings/core/tests/record.rs` (5) and `codec.rs` (4);
`python/tests/test_records.py` (7) and `test_identity.py` (9);
`npm/tests/records.test.mjs` (7) and `identity.test.mjs` (7). Positive tests run
bound records end to end: guards, actions, effects and outputs in declared
order, short-circuited condition records, dispositions, and preserved callback
causes. An adapter decodes IRIs through `MachineIdentityMaps` into a record,
binds and runs it, and encodes the outcome. Negative tests cover each rejection
listed above, codec duplicates with entry positions, limits, lazy consumption of
endless inputs, role mismatches, type-qualified keys and the absence of label or
`String()` fallbacks. The typing fixtures `python/tests/typing/records.py` and
`npm/tests/typing/records.mts` reject a wrong initial state, callables in place
of references, a missing registry, a codec of the wrong identity type and a
codec in the wrong role slot.

### Native composition

Both languages expose `InstanceAddress`, `Route`, `InstanceRegistry`,
`Composition`, `CapabilitySnapshot`, `BoundComposition`, `Delivery`,
`PropagationLimits`, `PropagationStatus` and the `Routed`/`Unmatched`/`Deferred`
items (discriminated by `kind`), plus `PropagationStep`, `AddressedEffect` and
`PropagationReport`. Python uses frozen dataclasses and an `Enum`; JavaScript
uses frozen classes and objects, with a TypeScript union over `kind` and a
report union that narrows `error` to `DispatchError` on `dispatch_failed`.

The kernel decides every rule through `status_machina_bridge::composition`:
`InstanceRegistry::builder`, `Composition::builder`, `route_batch`,
`CapabilitySnapshot::builder`, `Composition::bind` and
`BoundComposition::propagate`. Neither language selects routes, checks
signatures or orders work. Each composition owns a table of native addresses.
Python compares them type-qualified and JavaScript compares each component
with `Map` semantics, so an equal instance identity in another scope is another
address. A registry entry lowers to the kernel address of its machine only when
the address names the machine's `id`; otherwise the kernel reports
`AddressMismatch`. Addresses outside the table get call-local scopes, so the
kernel reports them as unregistered or unbound. Native readers consume at most
one entry beyond each declared bound and leave the bound to the kernel.

API decisions:

- `OutputEventIntent::new` is public in the Rust crate. Natively held output
  intents are rebuilt for `route_batch`. Routing never dispatches, and
  `Delivery::new` already accepts arbitrary events, so this widens nothing.
  `CapabilityReport` stays kernel-built: a native report keeps a hidden handle
  from `evaluate_capability`, and a snapshot rejects any other report.
- A capability snapshot belongs to the composition that lowered its addresses;
  another composition rejects it as `ForeignSnapshotError`.
- `bind` is exclusive like a Rust `&mut` borrow. Until the binding is disposed,
  bound machines refuse direct operations with `BusyError`, and binding one
  machine at two addresses is an `AliasedBindingError`. In Rust that is a borrow
  error at compile time.
- A route whose capability the destination definition does not declare fails
  as `UndeclaredCapabilityError` after the kernel accepted the routes, since
  native capability identities are definition-scoped.
- The kernel's `Composition::build` names no route. On rejection the bridge
  rebuilds growing prefixes to report the first rejected route. For
  `MissingInstance` it names the first registered address left unbound.
- Reports preserve the kernel's FIFO progress. The first `min(steps, inputs)`
  steps consumed the caller inputs, and unconsumed inputs lead `pending` as the
  original `Delivery` objects. Accepted steps survive a later failure, whose
  typed exception keeps the original callback cause.
- Payload mismatch and `RouteBuildError` are unrepresentable natively: all
  native payloads share one kernel type, and native routes require their
  destination and input event. They map to the base error classes.

Errors follow the kernel types: `InstanceRegistryError`,
`CompositionDefinitionError`, `RouteBatchError` (with `outputs`),
`CompositionBindingError`, `PropagationLimitsError` and
`CapabilitySnapshotError`, all under `CompositionError`, with one subclass per
reachable variant and the kernel `reason`, entry, route and native address in
their context. In Python the limit errors are also `LimitError`s.

Evidence: `bindings/core/tests/composition.rs` (6); `python/tests/test_composition.py`
(12) and `npm/tests/composition.test.mjs` (12), plus one example test in each
language. Positive tests propagate across machines with native payloads and
addressed effects, and resume from `pending` after a later failure. They
cover step and intent budgets, unbound destinations, scope isolation between
numerically equal enum scopes and between symbols, ordered batches and capability
deferral by stale, unavailable and available observations. Negative tests cover
every reachable registry, route, batch, binding, limit and snapshot rejection
with its context. They also cover exclusivity, reentrant propagation from a
callback and fabricated reports; in Node, cloning a composition, binding or
snapshot throws `DataCloneError`. The runnable `examples/composition.py` and
`examples/composition.mjs` propagate one order through three scopes and are shown
in the package READMEs. The typing fixtures `python/tests/typing/composition.py`
and `npm/tests/typing/composition.mts` reject raw identities as route endpoints,
missing budgets, non-integer budgets, bare payloads as deliveries and
definitions as registry entries.

### Native retry and waits

Both languages expose `RetryPolicy`, `FixedDelay`, `ExponentialBackoff`,
`RetryAt`/`RetryExhausted` (discriminated by `kind`), `WaitCorrelation`,
`WakeIntent` and `RetryWaits`. Python uses frozen dataclasses with
`datetime.timedelta` durations and timezone-aware `datetime` instants, returned
in UTC. JavaScript uses frozen classes, `Date` instants and a package `Duration`:
Node.js 22/24 ship no Temporal, and `number` milliseconds can neither carry Rust's
nanoseconds nor reach its range. `Duration` holds exact BigInt nanoseconds over
`0..=Duration::MAX`, which is its type invariant, as for Rust's `Duration`.
TypeScript narrows schedules by `kind` and keeps the instance, operation and
payload types of a wait set.

The kernel decides every rule through `status_machina_bridge::retry`:
`RetryPolicy::builder` and `schedule`, and `RetryWaits::builder`, `schedule`,
`pending`, `get`, `mark_registered`, `is_due`, `consume` and `cancel`. Neither
language computes delays, checks windows, assigns generations or matches wakes.
Durations and instants cross as decimal seconds plus nanoseconds, so the bridge
decides representability: negative or oversized durations, attempt counts
outside `u32`, generations outside `u64` and instants outside the platform's
`SystemTime` become `RetryRangeError`, never a clamp or a wrap.

API decisions:

- `WaitCorrelation::new` is public in the Rust crate. A reminder returned by an
  external scheduler must be rebuilt, and native correlations are values.
  Construction grants nothing: a wait set matches only the exact current
  instance, operation and generation, as for a clone.
- The elapsed window is required: `max_elapsed`/`maxElapsed` is a duration, or
  `None`/`null` for an explicitly indefinite window. The attempt bound is always
  finite. Omission is a type error in both languages.
- Each wait set interns its operations (type-qualified in Python, `Map`
  semantics in JavaScript) and lowers a correlation's instance to "this owner"
  or "another". An operation it never interned crosses as absent, so the kernel
  finds no match. Payloads stay in the language, keyed by the kernel-assigned
  generation, and leave with the consumed, cancelled or replaced wait. Both
  tables stay within the capacity.
- A wait set is machine data. `copy.deepcopy` and the Node default snapshot copy
  the kernel value and payloads, keeping identities; a custom Node snapshot uses
  `clone(copyPayload)`. Immutable retry values are shared by the Node snapshot.
  A Python wait set refuses pickling; a Node one is uncloneable.
- `Duration.after(date)` rounds up to the millisecond, so a JavaScript deadline
  is never early. Python adds a `timedelta` to a `datetime` natively.
- `GenerationExhausted` needs 2**64 schedules. It maps to
  `WaitGenerationExhaustedError`; only the kernel unit test reaches it.

Errors are `RetryPolicyError` (`ZeroDelayError`, `TooFewAttemptsError`,
`CapBelowInitialError`), `RetryWaitError` (`InvalidWaitCapacityError`,
`WaitCapacityReachedError`, `WaitGenerationExhaustedError`) and
`RetryRangeError`, all under `RetryError`, with the kernel `reason`, the `field`,
the native `value` and the capacity `limit` in their context. In Python the
capacity errors are also `LimitError`s and `RetryRangeError` is a `ValueError`.

Evidence: `tests/retry_waits.rs` (+1, rebuilt correlations) and
`bindings/core/tests/retry.rs` (5); `python/tests/test_retry.py` (16) and
`npm/tests/retry.test.mjs` (14), plus one example test in each language.
Positive tests cover fixed multi-hour delays, capped backoff, window exhaustion,
an indefinite window, saturating doubling, registration distinct from
consumption, replacement, rebuilt correlations, pre-epoch instants and
independent snapshots. Negative tests cover every reachable policy and wait
rejection, range errors with their field, elapsed-sum overflow (JavaScript,
where `Duration.MAX` is representable), early, stale, replaced, cancelled and
foreign wakes, including type-qualified and object-token instances. Machine
tests schedule and consume waits from candidate actions: duplicate deliveries
are ignored and never increase attempts, a capacity refusal rolls back the whole
transition with its typed cause, and window exhaustion takes the give-up
transition. The runnable `examples/retry.py` and `examples/retry.mjs` deliver
each wake twice and are shown in the package READMEs. The typing fixtures
`python/tests/typing/retry.py` and `npm/tests/typing/retry.mts` reject an omitted
window, numbers as durations or deadlines, bare durations as delay rules, wrong
payload types, tuples as correlations, numeric generations and a missing
capacity.

### Native scenario replay

Both languages expose `ScenarioLimits`, `ScenarioSet`, `ScenarioAssertion`,
`ScenarioReplay.run`, `ScenarioFailure`, `ScenarioLimit`, the step outcomes
(transitioned, ignored, refused, dispatch failed) and the four results
(`complete_within_scenarios`, `counterexample`, `limit_reached`,
`execution_failed`), all discriminated by `kind`, plus the observation, the
`CounterexampleTrace` and the `BoundedScenarioReport`. Python uses frozen
dataclasses, `Scenario`/`ScenarioStep` values and `Enum`s; JavaScript uses
frozen classes for limits, sets and assertions, checked plain objects for
scenarios and steps, frozen results and TypeScript unions over `kind`. Rust's
`ScenarioObservation::kind()` is `outcome` natively, because `kind` is the
discriminator of the result and outcome values.

The kernel decides every rule through `status_machina_bridge::scenario`:
`ScenarioLimits::builder`, `ScenarioSet::builder`, `ScenarioAssertion::builder`,
`BehaviorRegistry::resolve_assertion` and `ScenarioReplay::run`. Neither
language checks a bound, orders assertions, dispatches or trims a trace.
Construction checks run at native construction: limits through the builder,
an assertion predicate through the assertion builder, and set bounds by the set
builder over the step counts, in the kernel's own precedence. Native readers
consume at most one entry beyond each bound, and at most the kernel ceilings of
1024 scenarios, 4096 steps and 128 assertions, so endless iterables fail.

API decisions:

- `ScenarioLimits` gains public getters in the Rust crate
  (`max_scenarios()`, ..., `max_trace_entries()`). Native limits are values
  whose defaults the kernel resolves; the getters only expose the builder's
  result.
- A scenario crosses as its position, so two scenarios with equal native
  identities stay two traces. Assertions cross as positions too. States,
  events, transitions, dispositions, effects, Stories and implementations are
  indexes of the definition's own inventories; an undeclared event or an
  unknown Story crosses as an absent index, so the kernel refuses or rejects it.
  An observation's input event is the caller's own native event kind.
- One replay owns one value arena of the definition's `max_values`. Every
  input is leased before the first callback; equal input objects (Python
  identity, JavaScript `Map` semantics) share one lease, since callbacks only
  ever receive snapshots. Candidates are swept when the arena fills. Every
  lease is released when replay returns.
- A registered assertion keeps its definition callback; an inline assertion's
  callback follows the definition's callback table. A raised assertion,
  guard, action, constructor or snapshot stops replay (the kernel's
  `ScenarioReplayBuildError::EvaluationFailed`) as `CallbackError`/`SnapshotError`
  with its `phase` and original cause, never as a false assertion.
- An unresolved assertion reference raises the Story binding errors
  (`UnknownBehaviorError`, `UnknownImplementationError`, `RoleMismatchError`)
  naming the `assertion`, its `entry` and the native Story and implementation.

Errors are `ScenarioLimitsError`, `ScenarioSetError` (`EmptyScenarioSetError`,
`ScenarioCountLimitError`, `ScenarioStepLimitError`, `ScenarioNodeLimitError`),
`ScenarioAssertionError` (`MissingAssertionPredicateError`) and
`ScenarioReplayError` (`AssertionLimitError`), all under `ScenarioError`, with
the kernel `reason`, the `limit`, the refused scenario's `entry` and native
identity, and the limit values in their context. In Python the limit errors are
also `LimitError`s.

Evidence: `tests/scenario_replay.rs` (+1, limit getters) and
`bindings/core/tests/scenario.rs` (5); `python/tests/test_scenarios.py` (11) and
`npm/tests/scenarios.test.mjs` (11), plus one example test in each language.
Positive tests replay with native enum/symbol states and events, transitions,
dispositions, refused undeclared events and produced output and effect
identities, and count complete coverage (`scenarios`, `nodes`). They cover a
counterexample prefix through the failing step with ordered short-circuiting
assertions, fresh isolated snapshots per scenario, registered assertions,
output and trace budgets, a dispatch rejection traced as `dispatch_failed`,
invalid initial data at step 0, duplicate and type-distinct scenario
identities, and 200 shared-input steps in a 16-slot arena. Negative tests cover
every limit, set, assertion and replay rejection with its context, lazy bounds
on endless scenario, step and assertion iterables, wrong value types and unknown
fields, asynchronous and non-boolean assertions, and the original causes of
raised assertions, guards and snapshots. The runnable `examples/scenarios.py`
and `examples/scenarios.mjs` find an unpaid rush shipment and are shown in the
package READMEs. The typing fixtures `python/tests/typing/scenarios.py` and
`npm/tests/typing/scenarios.mts` narrow results by `kind` and reject bare
events as steps, plain objects as limits, non-integer limits, non-boolean
assertions, raw steps as a set, wrong data, a missing replay identity and, in
TypeScript, an assertion over another state type.

### Model certification

Each row was checked against the definition of done: native documentation,
type declarations, positive and negative tests in both languages and typed
errors. Suites now total 112 Python and 111 Node.js tests, plus 36 bridge tests.

Construction errors are typed by the kernel. `status_machina_bridge::rejection`
gives every kernel build error (`DefinitionBuildError`, `ConditionBuildError`,
`TransitionBuildError`, `DispositionBuildError`, `BindingError` and the
action, effect, output and capability builders) a native category and its
variant as `reason`, and the bridge attaches the rejected element (state,
transition, condition, disposition, action, effect, output event or
capability), resolved back to its native identity. The categories are
`UnknownStateError`, `DuplicateIdentifierError`, `UnreachableStateError`,
`TerminalTransitionError`, `InternalTransitionError`, `InvalidConditionError`,
`InvalidDispositionError`, `DefinitionLimitError`, `InitialInvariantError` and
`UnknownCapabilityError`, all `DefinitionError`s. `BindingError` is now a
`DefinitionError`, not a `RecordError`, because specifications, records and
scenario assertions all resolve Story references; it gains
`DuplicateImplementationError`. Before this, a specification's unresolved Story
reference or malformed condition surfaced as a bare `DefinitionError` message.
Definitions also read states, transitions, dispositions, condition operands,
phase lists and registry entries one entry past each bound and leave the bound
to the kernel, so `DefinitionLimitError` carries the kernel `reason` and limit.
Native nesting stops at 33 levels, where the kernel rejects the 33rd. Portable
records keep their own construction bounds.

| Row | Evidence |
| --- | --- |
| `MachineDefinition` and builders | Docs: "Define and run a machine" in both READMEs. Types: `Definition`, `State`, `Transition`, `Limits` (Python generics, `index.d.ts`), fixtures `typing/native_api`, `typing/contracts`. Tests: `test_definition_contract.py` / `definition-contract.test.mjs` (`DefinitionContractTests`: immutability, internal versus external self-transitions, every graph rule with its native identities, kernel-decided limits on endless inputs, the initial invariant), plus `test_native_api` and `lifecycle`. Errors: the construction categories above. |
| `Condition` | Docs: READMEs. Types: `Condition` unions. Tests: `ConditionContractTests` / condition tests in `definition-contract` (all/any/not short-circuit order, 32 levels accepted, 33 and 200 rejected, endless operands, empty groups, mixed expressions, raised and non-boolean predicates). Errors: `InvalidConditionError`, `DefinitionLimitError`, `CallbackError`. |
| `LocalAction`, `Effect`, `OutputEvent` | Docs: READMEs. Types: `Action` apply/update unions, `Effect`, `OutputEvent`, `EffectIntent`, `OutputIntent`. Tests: `IntentContractTests` / intent tests in `execution-contract` (exit, transition and entry phase order, payload classes retained, functional update, rollback of state, revision and data on a raised action, a returned value, a raised effect or output constructor, a postcondition or an invariant, phase limits), plus `test_lifecycle` effect short-circuiting. Errors: `CallbackError`, `PostconditionError`, `InvariantError`, `DefinitionLimitError`. |
| `BehaviorRef`, registry, roles, `BoundInvariant` | Docs: READMEs. Types: `BehaviorRef`, `BehaviorRegistry`, `BehaviorRole`. Tests: `StoryBindingContractTests` / binding tests in `execution-contract` (action, condition, effect and invariant roles resolved exactly; a bound invariant rejecting a candidate; unknown Story, unknown implementation, wrong role for a condition and an invariant, duplicate implementation, update mode on a reference), plus `test_records`, `test_capabilities` and `test_scenarios`. Errors: `BindingError` family. |
| `MachineInstance`, `Event`, decisions | Docs: READMEs. Types: `Machine`, `Decision`, `Event`, fixtures `typing/native_api`, `typing/contracts` (BigInt revisions, decisions not outcomes). Tests: `InstanceContractTests` / instance tests in `execution-contract` (stale `expected_revision` with both revisions, evaluate without effect until acceptance, single use, idempotent decision and machine disposal, disposed operations), plus `test_lifecycle`/`lifecycle` (foreign decisions, reentrancy, threads) and `workers`. Errors: `StaleDecisionError`, `ForeignDecisionError`, `DisposedError`, `BusyError`. |
| `DispatchOutcome`, `TransitionRecord`, intents | Docs: READMEs. Types: `Refused`/`Ignored`/`Transitioned` and the TypeScript `Outcome` union narrowed in `typing/contracts`. Tests: exact refused and transitioned outcomes with native instance identities, records and revisions in `execution-contract`; intent payload classes in the intent tests. |
| `EventDisposition` | Docs: READMEs. Types: `Disposition`, `Ignored`. Tests: `DispositionContractTests` / disposition tests in `definition-contract` (ignored versus refused with unchanged state, revision and data; a false condition refuses; invalid reasons; an overlapping transition is ambiguous; duplicate and unknown-state dispositions). Errors: `InvalidDispositionError`, `AmbiguousTransitionError`, `DuplicateIdentifierError`, `UnknownStateError`. |
| `Capability`, status, report, snapshot | Docs: READMEs, [native composition](#native-composition). Types: `Capability`, `CapabilityStatus`, `CapabilityReport`, `CapabilitySnapshot`. Tests: `test_capabilities`/`capabilities` (detached data, exact Story binding, raised policy is not denial, non-status results), `CapabilityContractTests` / capability tests in `execution-contract` (unknown identity names the native capability, duplicates), and the composition snapshot tests (stale, unavailable and available observations, fabricated and foreign reports). Errors: `UnknownCapabilityError`, `DuplicateIdentifierError`, `CapabilitySnapshotError` family. |
| Inspection and diagrams | Evidence listed under implementation evidence: `test_inspection`/`inspection` and `bindings/core/tests/inspection.rs` (coverage with native identities, equal labels kept distinct, escaping, entry, output and UTF-8 limits, invalid and missing labels, declaration order, no behavior evaluated), the order examples, `DiagramLabels`/`InspectionLimits` types. Errors: `InspectionError` with the violated budget. |
| Error taxonomy | Docs: "Errors" in both READMEs. Types: every class declared in `errors.py` and `index.d.ts`. Tests: `ErrorTaxonomyTests` / taxonomy test (every exported error is a `StatusMachinaError`; `BindingError` is a `DefinitionError`), `bindings/core/tests/rejection.rs` (3: graph rules, builder rules and Story references cross with category, variant and element), and the context assertions across every contract test. No construction error a native caller can cause crosses as text alone; the remaining message-only bridge errors are private transport invariants. |
| `EventOnDrop` | Implemented natively; the Rust type is a pure scope guard with no kernel state, so no bridge call is involved. Python `EventOnDrop` is a context manager; JavaScript `EventOnDrop` implements `dispose()` and `Symbol.dispose` for `using` (Node.js 24) or `finally` (Node.js 22). Docs: "Report abandoned operations on scope exit" in both READMEs. Types: `EventOnDrop[E]` / `EventOnDrop<E>` (fixtures `typing/contracts`). Tests: `EventOnDropContractTests` / EventOnDrop tests in `execution-contract` (delivery once on an exception after a correlation update, disarm is idempotent and cannot be rearmed, no delivery without scope exit, synchronous handlers returning nothing, a raised handler replacing the scope's exception). Errors: `TypeError` for asynchronous or value-returning handlers, `DisposedError` for rearming. |

BND-03 is Done: Python checked construction, native values, execution and
inspection through PyO3, typed source (`py.typed`, mypy fixtures) and the
exception taxonomy are evidenced on every CI runner and interpreter. Running
the same suites against installed wheels is BND-06 work.

BND-04 stays Partial. Every napi-rs construction, value, execution, inspection,
JavaScript and TypeScript clause above is evidenced, but each worker thread
loads the native addon from the package, and the worker isolation suite
(`npm/tests/workers.test.mjs`) runs against the checkout, not the installed npm
artifact. Those installed-artifact worker tests are a BND-06 deliverable; BND-04
closes when they pass.

BND-05 is Done: every parity row above is Covered with evidence in both
languages. Its dependency on BND-04 is met for every model clause; the only
open BND-04 clause is the installed-artifact worker evidence owned by BND-06.

Installed-artifact and platform evidence remain outstanding (BND-06 to BND-08).
These passing tests do not establish publication readiness.
