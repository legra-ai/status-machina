import {
  CapabilityStatus, Composition, Definition, Delivery, InstanceAddress, InstanceRegistry, PropagationLimits, Route,
  type DispatchError, type PropagationReport,
} from '@legra-ai/status-machina';

enum Phase { Open, Paid }
enum Input { Pay }
const orders = new Definition<Phase, Input, { paid: boolean }>({ id: 'orders', initial: Phase.Open,
  states: [{ id: Phase.Open }, { id: Phase.Paid }], transitions: [] });
const order = orders.instantiate({ id: 'order-1', data: { paid: false } });
const address = new InstanceAddress(Symbol('tenant'), 'order-1');
const scope: symbol = address.scope;
const instance: string = address.instance;
void [scope, instance];
const route = new Route({ source: address, event: 'paid', destination: address, inputEvent: Input.Pay });
const input: Input = route.inputEvent;
void input;
const composition = new Composition({ sources: new InstanceRegistry({ entries: [[address, order]] }),
  destinations: new InstanceRegistry({ entries: [[address, order]] }), routes: [route] });
const limits = new PropagationLimits({ maxSteps: 8, maxIntents: 8 });
using bound = composition.bind([[address, order]]);
const report: PropagationReport = bound.propagate([new Delivery(address, { kind: Input.Pay, payload: 1 })], { limits });
if (report.status === 'dispatch_failed') { const error: DispatchError = report.error; void error; }
for (const item of composition.routeBatch(address, [{ kind: 'paid', payload: 1 }])) {
  if (item.kind === 'deferred') { const status: (typeof CapabilityStatus)[keyof typeof CapabilityStatus] = item.status; void status; }
  if (item.kind === 'routed') { const destination: InstanceAddress = item.destination; void destination; }
}
// @ts-expect-error Route endpoints are InstanceAddress values, not raw identities.
new Route({ source: 'tenant', event: 'paid', destination: address, inputEvent: Input.Pay });
// @ts-expect-error Propagation budgets are explicit.
bound.propagate([]);
// @ts-expect-error Budgets are numbers.
new PropagationLimits({ maxSteps: '8', maxIntents: 8 });
// @ts-expect-error A batch source is an InstanceAddress.
composition.routeBatch('tenant', []);
// @ts-expect-error A delivery carries an event, not a bare payload.
new Delivery(address, 1);
// @ts-expect-error Registries hold machines, not definitions.
new InstanceRegistry({ entries: [[address, orders]] });
