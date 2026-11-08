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
| BND-03 | Python checked construction, native values, execution and inspection through PyO3, with typed Python source and exceptions | BND-02 | Partial |
| BND-04 | Node.js checked construction, native values, execution and inspection through napi-rs, with JavaScript and TypeScript APIs | BND-02 | Partial |
| BND-05 | Native Story bindings, capabilities, bounded composition, retries and scenario coverage, with model-by-model parity evidence in both languages | BND-03, BND-04 | Partial |
| BND-06 | Installable wheels, source distribution and npm artifacts; complete examples and installed-artifact tests without a Rust toolchain | BND-05 | Not started |
| BND-07 | Extend the shared tested-revision pipeline to gate and publish all three ecosystems, with synchronized versions and interrupted-publication recovery | BND-06 | Not started |
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

| Rust model family | Native counterpart and required evidence |
| --- | --- |
| `MachineIds`, `Identity`, `IdentityMap`, all identifier roles | Native role-aware identity inventories/codecs; enum/newtype/token round trips, no label-based equality |
| `MachineDefinition`, `DefinitionBuilder`, state/transition builders and kinds | Checked immutable specifications; initial state, terminal/internal/external rules, reachability and limits |
| `Condition`, expression kinds and builders | Native predicate and all/any/not expressions; bounded depth/operands, short-circuit and exception tests |
| `LocalAction`, `Effect`, `OutputEvent` | Typed native callbacks and returned intent values, phase order, owned payloads and exception rollback |
| `BehaviorRef`, registry, roles and `BoundInvariant` | Exact native Story plus implementation references; wrong role/missing implementation rejection |
| `DefinitionRecord`, state/transition/condition/disposition records, `BehaviorUse` | Native portable specification records and checked binding; no implicit code loading or string IDs |
| `MachineInstance`, `Event`, `MachineDecision`, `ValidatedDecision` | Native instance/event/single-use decision APIs; owned snapshots, explicit revision checks and disposal |
| `DispatchOutcome`, `TransitionRecord`, effect/output intents | Native discriminated outcomes and records retaining exact native identifiers and payload types |
| `EventDisposition` | Explicit harmless-event dispositions, conditions and reasons distinct from refusal |
| `Capability`, status/report/snapshot | Advisory native capability values and bounded checked snapshots; no hidden authorization authority |
| `Composition`, routes, registry/addresses, bound dispatch, batches/reports/limits | Explicit typed routing under caller control, multi-machine examples, ordered partial progress on failure, bounded propagation and namespace isolation |
| `RetryPolicy`, schedule, waits, correlation and wake intent | Native durations/correlations and caller-owned wake values; stale/foreign/cancelled wake tests; no scheduling |
| Scenario sets/replay/assertions/reports/traces/limits | Native replay and independent safety assertions; bounded counterexamples and coverage evidence |
| Inspection limits/coverage and diagram labels | Native inspection and Mermaid generation with application labels separate from identity; deterministic escaping and limit errors |
| All construction, binding, routing, dispatch, retry and inspection errors | Native exception taxonomy with typed context and original callback causes; no raw Rust `Result` or generic string-only errors |

Composition is not a distributed transaction: earlier accepted steps remain
accepted if a later delivery fails or hits a propagation bound. Reports must
preserve that progress and unconsumed work as defined by the kernel. Callback
failure still prevents acceptance of the failing step. Documentation and tests
must make this distinction visible.

## Packaging and release acceptance

Use dotenvage's PyO3/maturin and napi-rs package layout as implementation
references, retaining Status Machina's own license and current shared release
policy. Its older per-registry publisher scripts are not the release template.
Keep binding dependencies in unpublished workspace members, outside the core's
runtime dependency graph. Ship Python `py.typed`, typed source/stubs and native
exceptions; ship JavaScript plus TypeScript declarations and explicit exports.
Neither ordinary package installation may need Rust, Cargo, maturin or napi CLI.

Initial declared support follows a tested matrix: Python 3.10–3.14 and Node.js
22/24; Linux glibc x86_64/aarch64, macOS x86_64/aarch64, and Windows
x86_64/aarch64. Each advertised artifact needs build and installed-package test
evidence on its matching target. Add other interpreters, musl or architectures
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

BND-03 and BND-04 are Partial. The compiled Python extension passes 25 native
application and lifecycle tests; the Node addon passes 20. These cover enum/symbol
identities, domain data, detached snapshots, callback exceptions, checked decision
acceptance, disposal, bounded value-slot reuse and reentrancy. Python also tests
cross-thread rejection during evaluation and garbage collection of callback
cycles. Node tests both CommonJS and ESM entry points and cyclic typed-view data.

Python and TypeScript static examples check state/event identity, initial data
and data unions that functional actions can replace. They include deliberately
invalid calls which must remain type errors. Python source passes mypy.

Native capabilities include exact Story bindings, advisory reports tied to the
observed native instance/revision, detached data/facts and original exception
causes. Unknown and duplicate capability identifiers fail explicitly.

Composition, retry/scenario APIs, complete inspection and
installed-artifact/platform evidence remain outstanding. These passing tests do
not establish publication readiness or complete native-model parity.
