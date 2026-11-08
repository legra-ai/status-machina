# Status Machina for Node.js

Native JavaScript objects, TypeScript declarations, enum/symbol identities and
exceptions for caller-driven state machines. The transition kernel is shared
with the Rust and Python packages. Evaluation is synchronous; applications own
I/O, clocks, scheduling and external effect execution.

Development is in progress. This package has not been published yet.

The current implementation supports checked definitions, dispatch, isolated
snapshots, single-use decisions, exact callback causes and explicit disposal.
Full model parity, installed artifacts and the platform/release matrix remain
under development; see the repository's binding design.
