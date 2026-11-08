# Executable tutorial diagrams

These graphs are generated from the same definitions used by the runnable
[typed order](../../examples/typed_order.rs) and
[composed checkout](../../examples/checkout.rs) tutorials.
See the [tutorial guide](README.md) for the execution and ownership contracts.

## Guarded order

```mermaid
stateDiagram-v2
    state "Draft" as s0
    state "Accepted" as s1
    state "Shipped" as s2
    s2 --> [*]
    state "Cancelled" as s3
    s3 --> [*]
    [*] --> s0
    s0 --> s1 : Submit / Submit [guard: Stock available] (external)
    s1 --> s2 : Ship / Ship (external)
    s0 --> s3 : Cancel / Cancel (external)
```

## Invoice

```mermaid
stateDiagram-v2
    state "Idle" as s0
    state "Issued" as s1
    s1 --> [*]
    [*] --> s0
    s0 --> s1 : Issue invoice / Invoice requested (external)
```
