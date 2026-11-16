// Run with: node examples/scenarios.mjs (after installing @legra-ai/status-machina).
// Two scenarios replay an order machine against an independent safety assertion:
// an order never ships before it is paid. Replay finds the rush transition that
// skips payment and reports the bounded trace that falsified the assertion.
import { Definition, ScenarioAssertion, ScenarioLimits, ScenarioReplay, ScenarioSet } from '@legra-ai/status-machina';

const Phase = Object.freeze({ Cart: Symbol('CART'), Paid: Symbol('PAID'), Shipped: Symbol('SHIPPED') });
const Input = Object.freeze({ Pay: Symbol('PAY'), Ship: Symbol('SHIP'), Rush: Symbol('RUSH') });
const definition = new Definition({ id: 'orders', initial: Phase.Cart,
  states: [{ id: Phase.Cart }, { id: Phase.Paid }, { id: Phase.Shipped, terminal: true }],
  transitions: [
    { id: 'pay', source: Phase.Cart, event: Input.Pay, target: Phase.Paid,
      actions: [{ id: 'record', apply(data, amount) { data.paid.push(amount); } }] },
    { id: 'ship', source: Phase.Paid, event: Input.Ship, target: Phase.Shipped },
    { id: 'rush', source: Phase.Cart, event: Input.Rush, target: Phase.Shipped },
  ] });

const neverShipsUnpaid = new ScenarioAssertion({ id: 'never-ships-unpaid',
  evaluate: step => !(step.before === Phase.Cart && step.after === Phase.Shipped) });

function run(...scenarios) {
  const { result } = ScenarioReplay.run(definition, { id: 'replay', data: { paid: [] },
    scenarios: new ScenarioSet({ scenarios, limits: new ScenarioLimits({ maxStepsPerScenario: 8 }) }),
    assertions: [neverShipsUnpaid] });
  if (result.kind === 'complete_within_scenarios') console.log(`complete within ${result.scenarios} scenarios, ${result.nodes} steps`);
  else if (result.kind === 'counterexample') {
    console.log(`counterexample to ${result.trace.assertion} in ${result.trace.scenario}:`);
    for (const step of result.trace.steps) {
      console.log(`  ${step.step}: ${step.inputEvent.description} ${step.before.description} -> ${step.after.description}`);
    }
  }
}

const checkout = { id: 'checkout', steps: [{ event: { kind: Input.Pay, payload: 30 } }, { event: { kind: Input.Ship } }] };
run(checkout);
run(checkout, { id: 'rush', steps: [{ event: { kind: Input.Rush } }] });
