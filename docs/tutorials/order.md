The order begins in **Draft**. `Submit` is enabled only when the caller's
`Facts::stock_available` observation is true. A refused submission leaves its
state, data and revision unchanged. Once selected, the transition copies the
request into isolated `OrderData`; the Accepted state's invariant rejects a zero
quantity before acceptance. That failure also leaves the original instance
unchanged and releases no work.

Successful submission commits **Accepted**, returns `Work::SendConfirmation`,
and emits an `InvoiceRequested` output. Shipping later returns
`Work::DispatchParcel` and reaches terminal **Shipped**. Cancelling a draft
reaches terminal **Cancelled**. The kernel does not contact a warehouse, send
mail, or route the invoice output merely because a transition was accepted.

Run the complete example:

```sh
cargo run --example typed_order
```

Read the [executable](https://docs.rs/crate/status-machina/latest/source/examples/typed_order.rs)
and its [definition](https://docs.rs/crate/status-machina/latest/source/examples/tutorials/order.rs).
[`DispatchOutcome`] makes refusal, an intentional ignored event, and an accepted
transition distinct. Only the accepted outcome owns its effect and output
payloads. [`EffectIntent::into_parts`] transfers work to the caller without
requiring the request to implement `Clone`.

[`CandidateData::snapshot`] must produce independently mutable data. This example
contains only scalar fields, so copying is isolated; copying an `Arc<Mutex<_>>`
would not satisfy that contract. Payload size and callback cost remain bounded
caller responsibilities. Callbacks must not perform external effects.

## Composed checkout: two independent owners

The second tutorial connects the order's `InvoiceRequested` output to an invoice
instance. The invoice has its own data (`u64`), definition, state and revision;
its validated graph is shown below.
