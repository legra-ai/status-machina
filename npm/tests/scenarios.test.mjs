import assert from 'node:assert/strict';
import test from 'node:test';
import {
  AssertionLimitError, BehaviorRef, BehaviorRegistry, BehaviorRole, CallbackError, Definition, EmptyScenarioSetError,
  MissingAssertionPredicateError, RoleMismatchError, ScenarioAssertion, ScenarioCountLimitError, ScenarioError,
  ScenarioFailure, ScenarioLimit, ScenarioLimits, ScenarioLimitsError, ScenarioNodeLimitError, ScenarioReplay,
  ScenarioSet, ScenarioStepLimitError, SnapshotError, UnknownBehaviorError,
} from '@legra-ai/status-machina';

const Phase = Object.freeze({ Cart: Symbol('cart'), Paid: Symbol('paid'), Shipped: Symbol('shipped') });
const Input = Object.freeze({ Pay: Symbol('pay'), Ship: Symbol('ship'), Rush: Symbol('rush'), Undeclared: Symbol('undeclared') });
const neverUnpaid = Symbol('never unpaid');
const payStory = Symbol('pay');
const shippedUnpaid = observation => !(observation.before === Phase.Cart && observation.after === Phase.Shipped);
function definition({ rush = true, invariant, behaviors, limits } = {}) {
  const transitions = [
    { id: 'pay', source: Phase.Cart, event: Input.Pay, target: Phase.Paid,
      guard: { id: 'positive', predicate: (data, amount) => amount > 0 },
      actions: [{ id: 'record', apply: (data, amount) => { data.paid.push(amount); } }] },
    { id: 'ship', source: Phase.Paid, event: Input.Ship, target: Phase.Shipped,
      effects: [{ id: 'label', describe: () => 'label' }],
      outputs: [{ kind: 'shipped', describe: data => data.paid.reduce((a, b) => a + b, 0) }] },
  ];
  // The defect a safety assertion should find: rushing skips payment.
  if (rush) transitions.push({ id: 'rush', source: Phase.Cart, event: Input.Rush, target: Phase.Shipped });
  return new Definition({ id: 'orders', initial: Phase.Cart,
    states: [{ id: Phase.Cart }, { id: Phase.Paid }, { id: Phase.Shipped, terminal: true, invariant }], transitions,
    dispositions: [{ id: 'repeat', source: Phase.Paid, event: Input.Pay, condition: { id: 'any', predicate: () => true }, reason: 'already paid' }],
    behaviors, limits });
}
const step = (kind, payload, facts) => ({ event: { kind, payload }, facts });
const set = (scenarios, limits) => new ScenarioSet({ scenarios: scenarios.map(([id, steps]) => ({ id, steps })), limits });
const order = () => ({ paid: [] });
function* forever(make) { for (let n = 0; ; n += 1) yield make(n); }

test('complete replay observes native identities and counts its coverage', () => {
  const seen = [];
  const report = ScenarioReplay.run(definition(), { id: 'replay', data: order(),
    scenarios: set([['happy', [step(Input.Pay, 10), step(Input.Pay, 5), step(Input.Ship)]], ['stray', [step(Input.Undeclared)]]]),
    assertions: [new ScenarioAssertion({ id: 'record', evaluate: observation => { seen.push(observation); return true; } })] });
  assert.deepEqual(report.result, { kind: 'complete_within_scenarios', scenarios: 2, nodes: 4 });
  assert.ok(Object.isFrozen(report) && Object.isFrozen(report.result) && Object.isFrozen(seen[0]));
  assert.deepEqual(seen.map(o => [o.scenario, o.step, o.inputEvent, o.before, o.after]), [
    ['happy', 0, Input.Pay, Phase.Cart, Phase.Paid], ['happy', 1, Input.Pay, Phase.Paid, Phase.Paid],
    ['happy', 2, Input.Ship, Phase.Paid, Phase.Shipped], ['stray', 0, Input.Undeclared, Phase.Cart, Phase.Cart]]);
  assert.deepEqual(seen.map(o => o.outcome), [{ kind: 'transitioned', transition: 'pay' }, { kind: 'ignored', disposition: 'repeat' },
    { kind: 'transitioned', transition: 'ship' }, { kind: 'refused' }]);
  assert.deepEqual([seen[2].outputEvents, seen[2].effects], [['shipped'], ['label']]);
});
test('a counterexample is the bounded prefix through the failing step', () => {
  const later = [];
  const report = ScenarioReplay.run(definition(), { id: 'replay', data: order(),
    scenarios: set([['paid', [step(Input.Pay, 1), step(Input.Ship)]], ['rushed', [step(Input.Undeclared), step(Input.Rush)]],
      ['never', [step(Input.Pay, 1)]]]),
    assertions: [new ScenarioAssertion({ id: 'never-unpaid', evaluate: shippedUnpaid }),
      new ScenarioAssertion({ id: 'later', evaluate: o => { later.push(o.step); return true; } })] });
  assert.equal(report.result.kind, 'counterexample');
  const { trace } = report.result;
  assert.deepEqual([trace.scenario, trace.assertion], ['rushed', 'never-unpaid']);
  assert.deepEqual(trace.steps.map(o => [o.step, o.outcome]), [[0, { kind: 'refused' }], [1, { kind: 'transitioned', transition: 'rush' }]]);
  // Assertions run in order and stop at the first failure; the later scenario never runs.
  assert.deepEqual(later, [0, 1, 0]);
});
test('every scenario starts from an isolated snapshot, with registered assertions', () => {
  const original = { paid: [7] };
  const totals = [];
  const behaviors = new BehaviorRegistry({ implementations: [{ reference: new BehaviorRef(neverUnpaid, 'v1'),
    role: BehaviorRole.Assertion, callback: shippedUnpaid }] });
  const report = ScenarioReplay.run(definition({ behaviors, rush: false }), { id: 'replay', data: original,
    scenarios: set([['a', [step(Input.Pay, 3), step(Input.Ship)]], ['b', [step(Input.Pay, 4)]]]),
    assertions: [new ScenarioAssertion({ id: neverUnpaid, evaluate: new BehaviorRef(neverUnpaid, 'v1') }),
      new ScenarioAssertion({ id: 'totals', evaluate: o => { totals.push(o.outputEvents); return true; } })] });
  assert.deepEqual(report.result, { kind: 'complete_within_scenarios', scenarios: 2, nodes: 3 });
  assert.deepEqual(original.paid, [7]);
  assert.deepEqual(totals, [[], ['shipped'], []]);
});
test('dynamic budgets stop replay before the unchecked step', () => {
  let report = ScenarioReplay.run(definition(), { id: 'r', data: order(),
    scenarios: set([['a', [step(Input.Pay, 1), step(Input.Ship)]]], new ScenarioLimits({ maxOutputs: 1 })) });
  assert.deepEqual(report.result, { kind: 'limit_reached', scenario: 'a', step: 1, limit: ScenarioLimit.Outputs });
  report = ScenarioReplay.run(definition(), { id: 'r', data: order(),
    scenarios: set([['a', [step(Input.Pay, 1), step(Input.Pay, 1)]]], new ScenarioLimits({ maxTraceEntries: 1 })) });
  assert.deepEqual(report.result, { kind: 'limit_reached', scenario: 'a', step: 1, limit: ScenarioLimit.TraceEntries });
});
test('rejected dispatch and invalid initial data are execution failures', () => {
  const observed = [];
  let report = ScenarioReplay.run(definition({ invariant: () => false }), { id: 'r', data: order(),
    scenarios: set([['a', [step(Input.Pay, 1), step(Input.Ship)]]]),
    assertions: [new ScenarioAssertion({ id: 'watch', evaluate: o => { observed.push(o.outcome); return true; } })] });
  assert.deepEqual([report.result.kind, report.result.scenario, report.result.step, report.result.failure],
    ['execution_failed', 'a', 1, ScenarioFailure.InvariantFailed]);
  assert.deepEqual(report.result.steps.at(-1).outcome, { kind: 'dispatch_failed' });
  assert.equal(report.result.steps.at(-1).after, Phase.Paid);
  assert.deepEqual(observed.at(-1), { kind: 'dispatch_failed' });
  // Replay data that fails the initial state's invariant fails at step 0, with no steps.
  const guarded = new Definition({ id: 'd', initial: Phase.Cart, states: [{ id: Phase.Cart, invariant: data => data.paid.length === 0 }], transitions: [] });
  report = ScenarioReplay.run(guarded, { id: 'r', data: { paid: [1] }, scenarios: set([['a', [step(Input.Pay)]]]) });
  assert.deepEqual(report.result, { kind: 'execution_failed', scenario: 'a', step: 0, failure: ScenarioFailure.InvalidInitialData, steps: [] });
});
test('scenarios are positions and shared inputs fit a small value arena', () => {
  const facts = { shared: true }; const token = {};
  const scenarios = new ScenarioSet({ scenarios: [{ id: token, steps: Array(200).fill(step(Input.Undeclared, undefined, facts)) },
    { id: token, steps: [step(Input.Rush, undefined, facts)] }, { id: {}, steps: [step(Input.Rush)] }] });
  const report = ScenarioReplay.run(definition({ limits: { maxValues: 16 } }), { id: 'r', data: order(), scenarios,
    assertions: [new ScenarioAssertion({ id: 'never-unpaid', evaluate: shippedUnpaid })] });
  assert.equal(report.result.kind, 'counterexample');
  assert.equal(report.result.trace.scenario, token);
  assert.equal(report.result.trace.steps.length, 1);
});
test('limits are checked by the kernel', () => {
  assert.deepEqual({ ...new ScenarioLimits() }, { maxScenarios: 64, maxStepsPerScenario: 256, maxNodes: 4096, maxOutputs: 4096, maxTraceEntries: 4096 });
  for (const invalid of [{ maxScenarios: 0 }, { maxNodes: 65537 }, { maxTraceEntries: 2 ** 80 }, { maxOutputs: -1 }]) {
    assert.throws(() => new ScenarioLimits(invalid), error => error instanceof ScenarioLimitsError && error instanceof ScenarioError
      && error.context.reason === 'outside_supported_range' && error.context[Object.keys(invalid)[0]] === Object.values(invalid)[0]);
  }
  for (const wrong of [{ maxScenarios: true }, { maxNodes: 1.5 }, { maxRows: 1 }]) assert.throws(() => new ScenarioLimits(wrong), TypeError);
});
test('sets are bounded lazily by the kernel', () => {
  assert.throws(() => new ScenarioSet({ scenarios: [] }), EmptyScenarioSetError);
  assert.throws(() => new ScenarioSet({ scenarios: forever(n => ({ id: n, steps: [] })), limits: new ScenarioLimits({ maxScenarios: 2 }) }),
    error => error instanceof ScenarioCountLimitError && error.context.limit === 2 && error.context.scenario === 2);
  assert.throws(() => new ScenarioSet({ scenarios: [{ id: 'a', steps: forever(() => step(Input.Pay)) }] }),
    error => error instanceof ScenarioStepLimitError && error.context.limit === 256);
  assert.throws(() => set([['a', [step(Input.Pay), step(Input.Pay)]], ['b', [step(Input.Pay), step(Input.Pay)]]], new ScenarioLimits({ maxNodes: 3 })),
    error => error instanceof ScenarioNodeLimitError && error.context.limit === 3 && error.context.scenario === 'b' && error.context.entry === 1);
  assert.throws(() => new ScenarioSet({ scenarios: [['a', []]] }), TypeError);
  assert.throws(() => new ScenarioSet({ scenarios: [{ id: 'a', steps: [Input.Pay] }] }), TypeError);
  assert.throws(() => new ScenarioSet({ scenarios: [{ id: 'a', steps: [] }], limits: { maxNodes: 4 } }), TypeError);
  assert.throws(() => new ScenarioSet({ scenarios: [{ id: 'a', steps: [{ event: { kind: Input.Pay }, extra: 1 }] }] }), TypeError);
});
test('assertions need a synchronous predicate or an exact reference', () => {
  assert.throws(() => new ScenarioAssertion({ id: 'a' }), error => error instanceof MissingAssertionPredicateError
    && error.context.reason === 'missing_predicate' && error.context.assertion === 'a');
  for (const invalid of [true, async () => true]) assert.throws(() => new ScenarioAssertion({ id: 'a', evaluate: invalid }), TypeError);
});
test('assertion references resolve exactly or fail typed', () => {
  const behaviors = new BehaviorRegistry({ implementations: [{ reference: new BehaviorRef(payStory, 'v1'), role: BehaviorRole.Action,
    callback: () => undefined }] });
  const machine = definition({ behaviors }); const scenarios = set([['a', [step(Input.Pay, 1)]]]);
  assert.throws(() => ScenarioReplay.run(machine, { id: 'r', data: order(), scenarios, assertions: [
    new ScenarioAssertion({ id: 'fine', evaluate: shippedUnpaid }), new ScenarioAssertion({ id: 'missing', evaluate: new BehaviorRef(neverUnpaid, 'v1') })] }),
  error => error instanceof UnknownBehaviorError && error.context.story === neverUnpaid && error.context.implementation === 'v1'
    && error.context.entry === 1 && error.context.assertion === 'missing');
  assert.throws(() => ScenarioReplay.run(machine, { id: 'r', data: order(), scenarios,
    assertions: [new ScenarioAssertion({ id: 'pay', evaluate: new BehaviorRef(payStory, 'v1') })] }),
  error => error instanceof RoleMismatchError && error.context.expected === BehaviorRole.Assertion && error.context.actual === BehaviorRole.Action);
  assert.throws(() => ScenarioReplay.run(machine, { id: 'r', data: order(), scenarios,
    assertions: forever(n => new ScenarioAssertion({ id: n, evaluate: shippedUnpaid })) }),
  error => error instanceof AssertionLimitError && error.context.limit === 128);
  assert.throws(() => ScenarioReplay.run(machine, { id: 'r', data: order(), scenarios, assertions: [shippedUnpaid] }), TypeError);
  assert.throws(() => ScenarioReplay.run({}, { id: 'r', data: order(), scenarios }), TypeError);
  assert.throws(() => ScenarioReplay.run(machine, { data: order(), scenarios }), TypeError);
});
test('callback failures stop replay with their original cause', () => {
  const scenarios = set([['a', [step(Input.Pay, 1)]]]);
  const failure = new Error('assertion raised');
  assert.throws(() => ScenarioReplay.run(definition(), { id: 'r', data: order(), scenarios,
    assertions: [new ScenarioAssertion({ id: 'raising', evaluate: () => { throw failure; } })] }),
  error => error instanceof CallbackError && error.cause === failure && error.phase === 'assertion');
  assert.throws(() => ScenarioReplay.run(definition(), { id: 'r', data: order(), scenarios,
    assertions: [new ScenarioAssertion({ id: 'truthy', evaluate: () => 1 })] }),
  error => error instanceof CallbackError && error.cause instanceof TypeError);
  assert.throws(() => ScenarioReplay.run(definition(), { id: 'r', data: order(), scenarios,
    assertions: [new ScenarioAssertion({ id: 'promise', evaluate: () => Promise.resolve(true) })] }),
  error => error instanceof CallbackError && error.cause instanceof TypeError);
  const guard = new Error('guard raised');
  const broken = new Definition({ id: 'd', initial: Phase.Cart, states: [{ id: Phase.Cart }, { id: Phase.Paid }],
    transitions: [{ id: 't', source: Phase.Cart, event: Input.Pay, target: Phase.Paid, guard: { id: 'g', predicate: () => { throw guard; } } }] });
  assert.throws(() => ScenarioReplay.run(broken, { id: 'r', data: order(), scenarios }), error => error instanceof CallbackError && error.cause === guard);
  const snapshot = new Error('snapshot raised');
  const copying = new Definition({ id: 'd', initial: Phase.Cart, states: [{ id: Phase.Cart }], transitions: [], snapshot: () => { throw snapshot; } });
  assert.throws(() => ScenarioReplay.run(copying, { id: 'r', data: order(), scenarios }), error => error instanceof SnapshotError && error.cause === snapshot);
});
