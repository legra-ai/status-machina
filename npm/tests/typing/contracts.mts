import {
  BehaviorRef, Definition, DefinitionError, DefinitionLimitError, EventOnDrop, LimitError, UnknownBehaviorError,
  type BindingError, type Decision, type Event, type Outcome, type TransitionRecord,
} from '@legra-ai/status-machina';

enum Phase { Open, Closed }
enum Input { Close, Ping }
const definition = new Definition<Phase, Input, number[]>({ id: 'd', initial: Phase.Open,
  states: [{ id: Phase.Open }, { id: Phase.Closed, terminal: true }],
  transitions: [{ id: 't', source: Phase.Open, event: Input.Close, target: Phase.Closed,
    guard: { id: 'g', allOf: [{ id: 'a', predicate: () => true }] } }],
  dispositions: [{ id: 'p', source: Phase.Open, event: Input.Ping, condition: { id: 'c', predicate: () => true }, reason: 'dup' }] });
const machine = definition.instantiate({ id: 'm', data: [] });
const outcome: Outcome<Phase, Input> = machine.dispatch({ kind: Input.Close, payload: undefined }, undefined, { expectedRevision: 0n });
if (outcome.kind === 'transitioned') { const record: TransitionRecord<Phase> = outcome.record; const target: Phase = record.target; void target; }
else if (outcome.kind === 'ignored') { const reason: string = outcome.reason; void reason; }
else { const event: Input = outcome.event; void event; }
const decision: Decision = machine.evaluate({ kind: Input.Close, payload: undefined });
const expected: bigint = decision.expectedRevision;
void expected;
const reference = new BehaviorRef('story', 'v1');
void reference;
const error: DefinitionError = new UnknownBehaviorError('missing');
const binding: BindingError = new UnknownBehaviorError('missing');
const limit: LimitError | DefinitionError = new DefinitionLimitError('too many');
void [error, binding, limit];

{
  using guard = new EventOnDrop<Event<Input>>({ kind: Input.Close, payload: undefined }, event => { machine.dispatch(event); });
  const armed: Event<Input> | undefined = guard.event;
  void armed;
  guard.disarm();
}

// @ts-expect-error Revisions are BigInts.
machine.dispatch({ kind: Input.Close, payload: undefined }, undefined, { expectedRevision: 0 });
// @ts-expect-error An outcome is not a decision.
machine.accept(outcome);
// @ts-expect-error Events keep the definition's kind type.
machine.dispatch({ kind: 'close', payload: undefined });
// @ts-expect-error An internal flag is a boolean.
new Definition<Phase, Input>({ id: 'd', initial: Phase.Open, states: [{ id: Phase.Open }], transitions: [{ id: 't', source: Phase.Open, event: Input.Close, target: Phase.Open, internal: 'yes' }] });
// @ts-expect-error The armed event keeps its type.
new EventOnDrop<Event<Input>>({ kind: Input.Close, payload: undefined }, () => {}).event = 'abandoned';
// @ts-expect-error A handler receives exactly the event.
new EventOnDrop('x', (event: string, extra: number) => { void [event, extra]; });
