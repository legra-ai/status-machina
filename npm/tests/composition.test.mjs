import assert from 'node:assert/strict';
import test from 'node:test';
import {
  AddressMismatchError, AliasedBindingError, BatchLimitError, BindingMismatchError, BusyError, CallbackError,
  CapabilitySnapshotLimitError, CapabilityStatus, Composition, CompositionError, CompositionLimitError, Definition,
  Delivery, DisposedError, DuplicateBindingError, DuplicateInstanceError, DuplicateObservationError, DuplicateRouteError,
  ForeignSnapshotError, InstanceAddress, InstanceRegistry, InstanceRegistryLimitError, MissingBindingError,
  PropagationLimits, PropagationLimitsError, PropagationStatus, ReportInstanceMismatchError, Route, UndeclaredCapabilityError,
  UndeclaredRouteEventError, UnregisteredBindingError, UnregisteredRouteAddressError, UnregisteredSourceError,
} from '@legra-ai/status-machina';

const Order = Object.freeze({ Open: Symbol('open'), Paid: Symbol('paid') });
const Pay = Symbol('pay');
const Paid = Symbol('paid output');
const Audited = Symbol('audited output');
const Shipment = Object.freeze({ Idle: Symbol('idle'), Shipping: Symbol('shipping') });
const Ship = Symbol('ship');
const Ready = Symbol('ready');
class Payment { constructor(order, amount) { this.order = order; this.amount = amount; } }
const orders = new Definition({ id: 'orders', initial: Order.Open, states: [{ id: Order.Open }, { id: Order.Paid }],
  transitions: [{ id: 'pay', source: Order.Open, event: Pay, target: Order.Paid, outputs: [
    { kind: Paid, describe: (data, payload) => payload }, { kind: Audited, describe: (data, payload) => payload.order }] }],
  inputSnapshot: value => value });
const shipping = new Definition({ id: 'shipping', initial: Shipment.Idle, states: [{ id: Shipment.Idle }, { id: Shipment.Shipping }],
  transitions: [{ id: 'ship', source: Shipment.Idle, event: Ship, target: Shipment.Shipping,
    actions: [{ id: 'count', apply: (data, payload, facts) => { if (facts === 'fail') throw new RangeError('carrier offline'); data.shipped += 1; } }],
    effects: [{ id: 'label', describe: (data, payload) => ['label', payload] }] }],
  capabilities: [{ id: Ready, evaluate: (state, data, facts) => facts ? CapabilityStatus.Available : CapabilityStatus.Unavailable }],
  inputSnapshot: value => value });
const limits = new PropagationLimits({ maxSteps: 8, maxIntents: 8 });
const a = Symbol('tenant a');
function pipeline(scope = a, capability = undefined) {
  const order = orders.instantiate({ id: 'order-1', data: {} });
  const shipment = shipping.instantiate({ id: 'shipment-1', data: { shipped: 0 } });
  const source = new InstanceAddress(scope, 'order-1');
  const destination = new InstanceAddress(scope, 'shipment-1');
  const route = new Route({ source, event: Paid, destination, inputEvent: Ship, capability });
  const composition = new Composition({ sources: new InstanceRegistry({ entries: [[source, order]] }),
    destinations: new InstanceRegistry({ entries: [[destination, shipment]] }), routes: [route] });
  const bind = () => composition.bind([[source, order], [destination, shipment]]);
  // Node.js 22 has no `using` declarations; dispose explicitly.
  const within = body => { const bound = bind(); try { return body(bound); } finally { bound.dispose(); } };
  const pay = (amount = 10) => new Delivery(source, { kind: Pay, payload: new Payment('order-1', amount) });
  return { order, shipment, source, destination, route, composition, bind, within, pay };
}

test('outputs are routed FIFO with native payloads and addressed effects', () => {
  const p = pipeline();
  const payment = p.pay();
  const report = p.within(bound => bound.propagate([payment], { limits }));
  assert.equal(report.status, PropagationStatus.Quiescent);
  assert.deepEqual(report.steps.map(step => step.destination), [p.source, p.destination]);
  assert.equal(report.steps[1].outcome.record.target, Shipment.Shipping);
  assert.equal(report.effects.length, 1);
  assert.ok(report.effects[0].source.equals(p.destination));
  assert.equal(report.effects[0].intent.payload[1], payment.event.payload);
  assert.ok(report.effects[0].intent.payload[1] instanceof Payment);
  assert.deepEqual(report.unrouted.map(item => [item.kind, item.intent.kind, item.intent.payload]), [['unmatched', Audited, 'order-1']]);
  assert.equal(p.order.state, Order.Paid);
  assert.deepEqual(p.shipment.data, { shipped: 1 });
});

test('a later failure keeps accepted steps and resumes from pending', () => {
  const p = pipeline();
  const bound = p.bind();
  const report = bound.propagate([p.pay()], { limits, facts: 'fail' });
  assert.equal(report.status, PropagationStatus.DispatchFailed);
  assert.ok(report.error instanceof CallbackError);
  assert.ok(report.error.cause instanceof RangeError);
  assert.equal(report.steps.length, 1);
  assert.equal(report.pending.length, 1);
  assert.ok(report.pending[0].destination.equals(p.destination));
  assert.equal(report.pending[0].event.kind, Ship);
  const resumed = bound.propagate(report.pending, { limits, facts: 'ok' });
  bound.dispose();
  assert.equal(resumed.status, PropagationStatus.Quiescent);
  assert.deepEqual([p.order.revision, p.shipment.revision], [1n, 1n]);
});

test('budget stops return original inputs and unbound destinations', () => {
  const p = pipeline();
  const first = p.pay(1); const second = p.pay(2);
  p.within(bound => {
    let report = bound.propagate([first, second], { limits: new PropagationLimits({ maxSteps: 8, maxIntents: 1 }) });
    assert.equal(report.status, PropagationStatus.IntentLimit);
    assert.equal(report.steps.length, 0);
    assert.equal(report.pending[0], first);
    report = bound.propagate([first, second], { limits: new PropagationLimits({ maxSteps: 1, maxIntents: 8 }) });
    assert.equal(report.status, PropagationStatus.StepLimit);
    assert.equal(report.pending[0], second);
    assert.ok(report.pending[1].destination.equals(p.destination));
    const stray = new Delivery(new InstanceAddress(a, 'unknown'), { kind: Pay, payload: undefined });
    report = bound.propagate([stray], { limits });
    assert.equal(report.status, PropagationStatus.UnboundDestination);
    assert.equal(report.pending[0], stray);
  });
});

test('scopes isolate equal instance identities with Map semantics', () => {
  const tenant = pipeline(a); const other = pipeline(Symbol('tenant a'));
  assert.ok(!tenant.source.equals(other.source));
  assert.ok(tenant.source.equals(new InstanceAddress(a, 'order-1')));
  assert.throws(() => other.composition.bind([[tenant.source, other.order], [other.destination, other.shipment]]), UnregisteredBindingError);
  const report = tenant.within(bound => bound.propagate([other.pay()], { limits }));
  assert.equal(report.status, PropagationStatus.UnboundDestination);
  assert.deepEqual([tenant.order.revision, other.order.revision], [0n, 0n]);
});

test('routeBatch orders items and defers unobserved capabilities', () => {
  const p = pipeline(a, Ready);
  const outputs = [{ kind: Paid, payload: 7 }, { kind: Audited, payload: 8 }];
  const [deferred, unmatched] = p.composition.routeBatch(p.source, outputs);
  assert.equal(deferred.kind, 'deferred');
  assert.deepEqual([deferred.capability, deferred.status], [Ready, CapabilityStatus.Unknown]);
  assert.equal(deferred.intent, outputs[0]);
  assert.equal(unmatched.intent, outputs[1]);
  const report = p.shipment.evaluateCapability(Ready, true);
  const snapshot = p.composition.capabilities({ revisions: [[p.destination, 0n]], reports: [[p.destination, report]] });
  const [routed] = p.composition.routeBatch(p.source, outputs, { capabilities: snapshot });
  assert.deepEqual([routed.kind, routed.event.kind, routed.event.payload], ['routed', Ship, 7]);
  const stale = p.composition.capabilities({ revisions: [[p.destination, 1n]], reports: [[p.destination, report]] });
  assert.equal(p.composition.routeBatch(p.source, outputs, { capabilities: stale })[0].status, CapabilityStatus.Unknown);
});

test('propagation defers until the capability is observed', () => {
  const p = pipeline(a, Ready);
  const snapshot = p.composition.capabilities({ revisions: [[p.destination, 0n]],
    reports: [[p.destination, p.shipment.evaluateCapability(Ready, false)]] });
  const result = p.within(bound => bound.propagate([p.pay()], { limits, capabilities: snapshot }));
  const deferred = result.unrouted.filter(item => item.kind === 'deferred');
  assert.equal(deferred[0].status, CapabilityStatus.Unavailable);
  assert.equal(p.shipment.revision, 0n);
});

test('routeBatch rejections keep every output', () => {
  const p = pipeline();
  const outputs = [0, 1, 2].map(payload => ({ kind: Paid, payload }));
  assert.throws(() => p.composition.routeBatch(p.destination, outputs), error => error instanceof UnregisteredSourceError
    && error.outputs.length === 3 && error.outputs[0] === outputs[0]);
  const small = new Composition({ sources: new InstanceRegistry({ entries: [[p.source, p.order]] }),
    destinations: new InstanceRegistry({ entries: [[p.destination, p.shipment]] }), maxBatch: 2 });
  assert.throws(() => small.routeBatch(p.source, outputs.values()), error => error instanceof BatchLimitError && error.context.limit === 2);
  assert.throws(() => p.composition.routeBatch(p.source, [[Paid, 1]]), TypeError);
});

test('registry rejections name the entry, address and machine', () => {
  const p = pipeline();
  const wrong = new InstanceAddress(a, 'order-2');
  const empty = new InstanceRegistry();
  assert.throws(() => new Composition({ sources: new InstanceRegistry({ entries: [[wrong, p.order]] }), destinations: empty }),
    error => error instanceof AddressMismatchError && error.context.registry === 'sources' && error.context.entry === 0
      && error.context.machine === p.order && error.context.address === wrong);
  assert.throws(() => new Composition({ sources: new InstanceRegistry({ entries: [[p.source, p.order], [p.source, p.order]] }),
    destinations: empty }), DuplicateInstanceError);
  for (const registry of [new InstanceRegistry({ maxInstances: 0 }),
    new InstanceRegistry({ entries: [[p.source, p.order], [p.destination, p.shipment]], maxInstances: 1 })]) {
    assert.throws(() => new Composition({ sources: registry, destinations: empty }), InstanceRegistryLimitError);
  }
});

test('route rejections name the native route', () => {
  const p = pipeline();
  const sources = new InstanceRegistry({ entries: [[p.source, p.order]] });
  const destinations = new InstanceRegistry({ entries: [[p.destination, p.shipment]] });
  const cases = [
    [DuplicateRouteError, [p.route, p.route], 1],
    [UnregisteredRouteAddressError, [new Route({ source: p.source, event: Paid, destination: new InstanceAddress(a, 'ghost'), inputEvent: Ship })], 0],
    [UndeclaredRouteEventError, [p.route, new Route({ source: p.source, event: Pay, destination: p.destination, inputEvent: Ship })], 1],
  ];
  for (const [Type, routes, index] of cases) {
    assert.throws(() => new Composition({ sources, destinations, routes }),
      error => error instanceof Type && error instanceof CompositionError && error.context.route === routes[index]);
  }
  assert.throws(() => new Composition({ sources, destinations, maxBatch: 0 }), CompositionLimitError);
  assert.throws(() => new Composition({ sources, destinations, routes: [new Route({ source: p.source, event: Paid,
    destination: p.destination, inputEvent: Ship, capability: 'unknown' })] }), UndeclaredCapabilityError);
});

test('binding is exclusive, complete and unaliased', () => {
  const p = pipeline();
  assert.throws(() => p.composition.bind([[p.source, p.order]]), error => error instanceof MissingBindingError && error.context.address === p.destination);
  assert.throws(() => p.composition.bind([[p.source, p.shipment], [p.destination, p.order]]), BindingMismatchError);
  assert.throws(() => p.composition.bind([[p.source, p.order], [p.source, orders.instantiate({ id: 'order-1', data: {} })]]), DuplicateBindingError);
  assert.throws(() => p.composition.bind([[p.source, p.order], [p.source, p.order]]), AliasedBindingError);
  const twin = new InstanceAddress('audit', 'order-1');
  const both = new Composition({ sources: new InstanceRegistry({ entries: [[p.source, p.order], [twin, p.order]] }), destinations: new InstanceRegistry() });
  assert.throws(() => both.bind(new Map([[p.source, p.order], [twin, p.order]])), AliasedBindingError);
  const bound = p.bind();
  for (const value of [p.composition, bound, p.composition.capabilities()]) {
    assert.throws(() => structuredClone(value), error => error.name === 'DataCloneError');
  }
  assert.throws(() => p.order.dispatch({ kind: Pay, payload: undefined }), BusyError);
  assert.throws(() => p.shipment.dispose(), BusyError);
  assert.throws(() => p.bind(), BusyError);
  bound.dispose(); bound.dispose();
  assert.throws(() => bound.propagate([], { limits }), DisposedError);
  assert.equal(p.order.state, Order.Open);
});

test('reentrant propagation fails the step without acceptance', () => {
  const holder = {};
  const looping = new Definition({ id: 'loop', initial: Order.Open, states: [{ id: Order.Open }, { id: Order.Paid }],
    transitions: [{ id: 'pay', source: Order.Open, event: Pay, target: Order.Paid,
      actions: [{ id: 'reenter', apply: () => { holder.bound.propagate([], { limits }); } }] }] });
  const machine = looping.instantiate({ id: 'loop', data: {} });
  const address = new InstanceAddress(a, 'loop');
  const composition = new Composition({ sources: new InstanceRegistry(), destinations: new InstanceRegistry({ entries: [[address, machine]] }) });
  const bound = composition.bind([[address, machine]]);
  holder.bound = bound;
  const report = bound.propagate([new Delivery(address, { kind: Pay, payload: undefined })], { limits });
  bound.dispose();
  assert.ok(report.error.cause instanceof BusyError);
  assert.equal(machine.revision, 0n);
});

test('limits and snapshots are kernel-checked', () => {
  for (const [maxSteps, maxIntents] of [[0, 1], [1, 4097], [-1, 1], [2 ** 40, 1]]) {
    assert.throws(() => new PropagationLimits({ maxSteps, maxIntents }), PropagationLimitsError);
  }
  assert.throws(() => new PropagationLimits({ maxSteps: '1', maxIntents: 1 }), TypeError);
  const p = pipeline(a, Ready); const other = pipeline();
  const report = p.shipment.evaluateCapability(Ready, true);
  assert.throws(() => p.composition.capabilities({ reports: [[p.source, report]] }), ReportInstanceMismatchError);
  assert.throws(() => p.composition.capabilities({ revisions: [[p.destination, 0n], [p.destination, 0n]] }), DuplicateObservationError);
  assert.throws(() => p.composition.capabilities({ revisions: [[p.source, 0n], [p.destination, 0n]], maxObservations: 1 }), CapabilitySnapshotLimitError);
  assert.throws(() => p.composition.capabilities({ reports: [[p.destination, { ...report }]] }), TypeError);
  assert.throws(() => other.composition.routeBatch(other.source, [], { capabilities: p.composition.capabilities() }), ForeignSnapshotError);
});
