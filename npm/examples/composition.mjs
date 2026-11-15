// Run with: node examples/composition.mjs (after installing @legra-ai/status-machina).
import {
  Composition, Definition, Delivery, InstanceAddress, InstanceRegistry, PropagationLimits, Route,
} from '@legra-ai/status-machina';

const Order = Object.freeze({ Open: 'OPEN', Paid: 'PAID' });
const Shipment = Object.freeze({ Waiting: 'WAITING', Packed: 'PACKED' });
const Invoice = Object.freeze({ Draft: 'DRAFT', Issued: 'ISSUED' });
const Pay = Symbol('pay'); const Paid = Symbol('paid');
const Pack = Symbol('pack'); const Packed = Symbol('packed');
const Issue = Symbol('issue');
const forward = (data, payload) => payload;

const orders = new Definition({ id: 'orders', initial: Order.Open, states: [{ id: Order.Open }, { id: Order.Paid }],
  transitions: [{ id: 'pay', source: Order.Open, event: Pay, target: Order.Paid, outputs: [{ kind: Paid, describe: forward }] }] });
const shipping = new Definition({ id: 'shipping', initial: Shipment.Waiting,
  states: [{ id: Shipment.Waiting }, { id: Shipment.Packed }],
  transitions: [{ id: 'pack', source: Shipment.Waiting, event: Pack, target: Shipment.Packed,
    effects: [{ id: 'print-label', describe: (data, payload) => `label for ${payload.order}` }],
    outputs: [{ kind: Packed, describe: forward }] }] });
const billing = new Definition({ id: 'billing', initial: Invoice.Draft, states: [{ id: Invoice.Draft }, { id: Invoice.Issued }],
  transitions: [{ id: 'issue', source: Invoice.Draft, event: Issue, target: Invoice.Issued,
    actions: [{ id: 'book', apply: (data, payload) => { data.total += payload.amount; } }] }] });

// One business identity in three routing scopes: three distinct addresses.
const order = orders.instantiate({ id: 'order-7', data: {} });
const shipment = shipping.instantiate({ id: 'order-7', data: {} });
const invoice = billing.instantiate({ id: 'order-7', data: { total: 0 } });
const [sales, warehouse, finance] = ['sales', 'warehouse', 'finance'].map(scope => new InstanceAddress(scope, 'order-7'));
const composition = new Composition({
  sources: new InstanceRegistry({ entries: [[sales, order], [warehouse, shipment]] }),
  destinations: new InstanceRegistry({ entries: [[warehouse, shipment], [finance, invoice]] }),
  routes: [new Route({ source: sales, event: Paid, destination: warehouse, inputEvent: Pack }),
    new Route({ source: warehouse, event: Packed, destination: finance, inputEvent: Issue })],
});
const bound = composition.bind([[sales, order], [warehouse, shipment], [finance, invoice]]);
const report = bound.propagate([new Delivery(sales, { kind: Pay, payload: { order: 'order-7', amount: 120 } })],
  { limits: new PropagationLimits({ maxSteps: 8, maxIntents: 8 }) });
bound.dispose();
// The binding is released: execute effects now, outside any machine operation.
for (const step of report.steps) console.log(`${step.destination.scope}: ${step.outcome.record.source} -> ${step.outcome.record.target}`);
for (const effect of report.effects) console.log(`${effect.source.scope} effect: ${effect.intent.payload}`);
console.log(report.status, JSON.stringify(invoice.data));
