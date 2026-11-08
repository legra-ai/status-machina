The order does not invoke the invoice from inside a transition. The caller
registers both actual instances and declares one [`Route`]: the order's
`InvoiceRequested` output becomes the invoice's `InvoiceRequested` input.
Instance addresses include a caller-owned scope and instance identity.

A [`BoundComposition`] temporarily borrows both instances exclusively. One
[`BoundComposition::propagate`] call receives the initial `Submit` delivery and
explicit budgets of two dispatch steps and four retained intents. It accepts the
order, routes its output, then accepts the invoice. The returned report contains
two steps and two owned effects: confirmation and invoice delivery. The example
drops the binding before returning those effects for external execution.

Run the complete example:

```sh
cargo run --example checkout
```

Read the [executable](https://docs.rs/crate/status-machina/latest/source/examples/checkout.rs),
[composition and invoice definition](https://docs.rs/crate/status-machina/latest/source/examples/tutorials/checkout.rs),
and [shared domain types](https://docs.rs/crate/status-machina/latest/source/examples/tutorials/model.rs).

Propagation is synchronous and bounded, not a background orchestration service.
A step or intent limit returns remaining deliveries to the caller. A later
machine failure does not roll back earlier accepted transitions, and returned
effects have not been executed. [`PropagationReport`] distinguishes accepted
steps, pending deliveries, unrouted outputs and external effects.

## Further exploration

The [GitHub tutorial guide](https://github.com/legra-ai/status-machina/blob/main/docs/tutorials/README.md)
contains the same generated graphs as Mermaid, which GitHub renders directly.
The [diagram gallery](https://github.com/legra-ai/status-machina/blob/main/docs/diagrams.md)
also covers the EKGF execution lifecycle and retry/duplicate-completion graph.
Use [`RetryPolicy`] and [`RetryWaits`] for correlated retry intent; the caller
continues to own scheduling and wake delivery. Behaviors referenced through
[`BehaviorRef`] correspond to Stories in the EKGF Story Ontology; Story identity
and executable implementation identity remain separate.

The images above are inline SVG, so Rustdoc and docs.rs display them without a
JavaScript renderer or an external image request. Their Mermaid source comes
from validated definitions; the checked-in SVGs retain that exact source and
regression tests reject graph drift. These additions appear with the next crate
release; previously published documentation remains immutable.
