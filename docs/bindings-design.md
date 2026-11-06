# Python and Node binding scope

Python and Node bindings are useful for configuring, inspecting and driving machines from application code. The Rust core remains independent of either runtime. Use the existing PyO3/maturin and napi-rs packaging approach as the implementation reference.

The first binding should expose checked definitions, instance dispatch, accepted decisions, typed effect/output records, composition and diagram generation. Callers retain ownership of clocks, I/O, persistence, scheduling and serialized access to each instance.

## Identity

Python Enum values and TypeScript discriminated types should have explicit codecs to the core identity family. Wire strings or numbers are representations, not universal identities. Preserve distinct roles for state, event, Story and implementation identifiers. Do not turn callbacks into dynamically loaded code or equate a label with implementation identity.

## Behavior boundary

Rust generics and captured closures cannot simply be exported as arbitrary cross-language values. Decide the supported data/event/fact payload model and callback registration contract before implementation. Validate Story implementation selection before instantiation. Python callbacks require a defined interpreter/GIL boundary; Node callbacks require an explicit execution-thread and lifetime contract. Neither may silently introduce async callbacks into synchronous candidate evaluation.

Candidate data must support isolated snapshots. Errors and failed invariants must leave accepted state unchanged. Effects are returned after acceptance and executed by the language caller. Cross-runtime exceptions must become explicit dispatch errors without partial acceptance.

## Delivery evidence

Each language needs tests for Enum/discriminated identifiers, snapshots, rejected transitions, callback exceptions, exact implementation references, explicit routing, bounded inputs and disposal. Build and test actual wheels/npm packages on supported platforms through the shared tested-revision release pipeline. Support and package names must be selected before publication.

Bindings are a separate deliverable. This document records design constraints; no Python or Node implementation is included in the initial Rust extraction candidate.
