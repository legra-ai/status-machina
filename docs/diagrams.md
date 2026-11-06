# Generated state-machine diagrams

These Mermaid blocks are generated from validated Rust machine definitions.
GitHub renders them directly when viewing this Markdown file.

Regenerate from the repository root:

```sh
cargo run --quiet --package state-machine --example diagrams > crates/state-machine/docs/diagrams.md
```

The `generated_diagrams` test checks this file against the generator.
[Generator source](../examples/diagrams/model.rs) uses the convenience identity
family for readable labels; custom families can use enums and `DiagramLabels`.
Layout is chosen by GitHub's Mermaid renderer. These examples do not require
a particular Mermaid version or layout engine.

## EKGF execution lifecycle

Six states and seven permitted routes, based on the
EKGF execution lifecycle model.
Event identities are explicit example choices. Initial state is Admitted;
Done, Failed and Cancelled are terminal.

```mermaid
stateDiagram-v2
    state "Admitted" as s0
    state "Cancelled" as s1
    s1 --> [*]
    state "Done" as s2
    s2 --> [*]
    state "Failed" as s3
    s3 --> [*]
    state "Queued" as s4
    state "Running" as s5
    [*] --> s0
    s0 --> s1 : AdmittedToCancelled / Cancel (external)
    s0 --> s4 : AdmittedToQueued / Queue (external)
    s4 --> s1 : QueuedToCancelled / Cancel (external)
    s4 --> s5 : QueuedToRunning / Start (external)
    s5 --> s1 : RunningToCancelled / Cancel (external)
    s5 --> s2 : RunningToDone / Complete (external)
    s5 --> s3 : RunningToFailed / Fail (external)
```

## Retry and duplicate completion

A small illustrative machine shows a guarded internal self-transition, an
accepted completion and an explicit harmless duplicate-completion disposition.
Its unit payloads and constant predicates illustrate graph structure; a real
consumer supplies attempt correlation and policy facts.

```mermaid
stateDiagram-v2
    state "Done" as s0
    s0 --> [*]
    state "Waiting" as s1
    [*] --> s1
    s1 --> s0 : AcceptCompletion / Completed (external)
    s1 --> s1 : Retry / RetryDue [guard: AttemptCurrent] (internal)
    s0 --> s0 : disposition: IgnoreDuplicate / Completed [guard: AlreadyAccepted] completion already accepted
```

