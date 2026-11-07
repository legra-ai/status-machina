# Python and Node binding scope

Python and Node bindings are useful for configuring, inspecting and driving machines from application code. The Rust core remains independent of either runtime. Use the existing PyO3/maturin and napi-rs packaging approach as the implementation reference.

The first binding should expose checked definitions, instance dispatch, accepted decisions, typed effect/output records, composition and diagram generation. Callers retain ownership of clocks, I/O, persistence, scheduling and serialized access to each instance.

## Native language experience

Users must not need Rust knowledge. Python exposes native Enum classes, typed dataclasses, type hints, Python exceptions, context-managed resource ownership and Python documentation. Node exposes normal JavaScript objects and errors, TypeScript discriminated unions/enums and declaration files, with explicit disposal and event-delivery semantics. All public Rust model roles need reviewed native equivalents; do not expose opaque Rust handles where domain values are appropriate.

Construction, inspection, errors, return values and callback contracts must follow each language's conventions. Validate wheel/npm installation and complete examples without a Rust toolchain. The native interface is a product API, not a mechanical translation of Rust generic parameters or Result values.

## Selected package names

The owner selected Status Machina on 2026-11-07:

- Rust crate: `status-machina`; Rust import: `status_machina`.
- Python distribution: `status-machina`; Python import: `status_machina`.
- npm package: `@legra-ai/status-machina`.
- Repository: `legra-ai/status-machina`.

The unscoped npm name `status-machina` is occupied by an existing state-machine package. Scoped npm naming avoids that collision. Rust/PyPI and the scoped npm name had no registry entries when checked; these checks do not reserve names or constitute publication.

## Identity

Python Enum values and TypeScript discriminated types should have explicit codecs to the core identity family. Wire strings or numbers are representations, not universal identities. Preserve distinct roles for state, event, Story and implementation identifiers. Do not turn callbacks into dynamically loaded code or equate a label with implementation identity.

## Behavior boundary

Rust generics and captured closures cannot simply be exported as arbitrary cross-language values. Decide the supported data/event/fact payload model and callback registration contract before implementation. Validate Story implementation selection before instantiation. Python callbacks require a defined interpreter/GIL boundary; Node callbacks require an explicit execution-thread and lifetime contract. Neither may silently introduce async callbacks into synchronous candidate evaluation.

Candidate data must support isolated snapshots. Errors and failed invariants must leave accepted state unchanged. Effects are returned after acceptance and executed by the language caller. Cross-runtime exceptions must become explicit dispatch errors without partial acceptance.

## Delivery evidence

Each language needs tests for Enum/discriminated identifiers, snapshots, rejected transitions, callback exceptions, exact implementation references, explicit routing, bounded inputs and disposal. Build and test actual wheels/npm packages on supported platforms through the shared tested-revision release pipeline. Support and package names must be selected before publication.

Bindings are a separate deliverable. This document records design constraints; no Python or Node implementation is included in the initial Rust extraction candidate.
