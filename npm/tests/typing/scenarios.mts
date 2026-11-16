import {
  Definition, ScenarioAssertion, ScenarioLimits, ScenarioReplay, ScenarioSet,
  type BoundedScenarioReport, type CounterexampleTrace, type ScenarioLimitValue, type ScenarioObservation,
  type ScenarioStepKind,
} from '@legra-ai/status-machina';

enum Phase { Open, Closed }
enum Input { Close }
const definition = new Definition<Phase, Input, number[]>({ id: 'd', initial: Phase.Open,
  states: [{ id: Phase.Open }, { id: Phase.Closed, terminal: true }],
  transitions: [{ id: 't', source: Phase.Open, event: Input.Close, target: Phase.Closed }] });
const scenarios = new ScenarioSet<string, Input>({ scenarios: [{ id: 'close', steps: [{ event: { kind: Input.Close, payload: undefined } }] }],
  limits: new ScenarioLimits({ maxTraceEntries: 16 }) });
const staysOpen = new ScenarioAssertion({ id: 'stays-open', evaluate: (observation: ScenarioObservation<string, Phase, Input>) => {
  const after: Phase = observation.after; const event: Input = observation.inputEvent; const outcome: ScenarioStepKind = observation.outcome;
  void [event, outcome];
  return after === Phase.Open;
} });
const report: BoundedScenarioReport<string, Phase, Input> = ScenarioReplay.run(definition, { id: 'replay', data: [], scenarios, assertions: [staysOpen] });
const result = report.result;
if (result.kind === 'counterexample') { const trace: CounterexampleTrace<string, Phase, Input> = result.trace; const before: Phase = trace.steps[0].before; void before; }
else if (result.kind === 'limit_reached') { const limit: ScenarioLimitValue = result.limit; const scenario: string = result.scenario; void [limit, scenario]; }
else if (result.kind === 'complete_within_scenarios') { const nodes: number = result.nodes; void nodes; }

// @ts-expect-error A step needs an event object, not a bare kind.
new ScenarioSet<string, Input>({ scenarios: [{ id: 'close', steps: [Input.Close] }] });
// @ts-expect-error Limits are ScenarioLimits, not a plain object.
new ScenarioSet({ scenarios: [], limits: { maxNodes: 4 } });
// @ts-expect-error Limits are numbers.
new ScenarioLimits({ maxNodes: '4' });
// @ts-expect-error Assertions return booleans.
new ScenarioAssertion({ id: 'a', evaluate: () => 'open' });
// @ts-expect-error Scenarios must be a ScenarioSet, not raw steps.
ScenarioReplay.run(definition, { id: 'replay', data: [], scenarios: [{ event: { kind: Input.Close } }] });
// @ts-expect-error Data keeps the definition's type.
ScenarioReplay.run(definition, { id: 'replay', data: 'text', scenarios });
// @ts-expect-error The replay instance identity is explicit.
ScenarioReplay.run(definition, { data: [], scenarios });
// @ts-expect-error An assertion over another state type does not fit this definition.
ScenarioReplay.run(definition, { id: 'replay', data: [], scenarios, assertions: [new ScenarioAssertion({ id: 'x', evaluate: (o: ScenarioObservation<string, string, Input>) => o.after === 'open' })] });
