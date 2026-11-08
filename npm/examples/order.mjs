/** Run with: node examples/order.mjs (after building/installing status-machina). */
import { Definition } from '@legra-ai/status-machina';
export const Phase = Object.freeze({ Pending: Symbol('pending'), Approved: Symbol('approved') });
export const Input = Object.freeze({ Approve: Symbol('approve') });
const step = Symbol('approve order'); const policy = Symbol('positive total'); const change = Symbol('mark approved');
export class Order {
  constructor(total, approved = false) { this.total = total; this.approved = approved; }
}
export function definition() {
  return new Definition({ id: 'Order approval', initial: Phase.Pending,
    snapshot: data => new Order(data.total, data.approved),
    states: [{ id: Phase.Pending }, { id: Phase.Approved, terminal: true }],
    transitions: [{ id: step, source: Phase.Pending, event: Input.Approve, target: Phase.Approved,
      guard: { id: policy, predicate: data => data.total > 0n },
      actions: [{ id: change, apply: data => { data.approved = true; } }] }] });
}
export function diagram() {
  return definition().diagram({ states: new Map([[Phase.Pending, 'Pending'], [Phase.Approved, 'Approved']]),
    transitions: new Map([[step, 'Approve order']]), events: new Map([[Input.Approve, 'Approve']]),
    conditions: new Map([[policy, 'Positive total']]) });
}
const machine = definition().instantiate({ id: Symbol('order'), data: new Order(120n) });
try {
  const outcome = machine.dispatch({ kind: Input.Approve, payload: undefined });
  if (outcome.kind !== 'transitioned' || machine.state !== Phase.Approved || !machine.data.approved) {
    throw new Error('Order approval example failed');
  }
} finally { machine.dispose(); }
process.stdout.write(diagram());
