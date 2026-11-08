import { Definition, CapabilityStatus, type Capability, type CapabilityReport, type Machine } from '@legra-ai/status-machina';

enum Phase { Ready, Done }
enum Input { Finish }
enum Wrong { Finish }
const definition = new Definition<Phase, Input, number | null>({
  id: Symbol('definition'), initial: Phase.Ready,
  states: [{ id: Phase.Ready }, { id: Phase.Done, terminal: true }],
  transitions: [{ id: Symbol('transition'), source: Phase.Ready, event: Input.Finish,
    target: Phase.Done, actions: [{ id: Symbol('action'), update: () => null }] }],
});
const machine: Machine<Phase, Input, number | null> = definition.instantiate({ id: Symbol(), data: 1 });
const phase: Phase = machine.state;
const data: number | null = machine.data;
const revision: bigint = machine.revision;
machine.dispatch({ kind: Input.Finish, payload: undefined });
// @ts-expect-error Event enum identity belongs to this definition.
machine.dispatch({ kind: Wrong.Finish, payload: undefined });
// @ts-expect-error Initial data must satisfy the declared data type.
definition.instantiate({ id: Symbol(), data: 'wrong' });
// @ts-expect-error A functional update can replace the initial number with null.
const narrowedData: number = machine.data;
void [phase, data, revision, narrowedData];

const policy: Capability<Phase, number | null> = { id: Symbol(), evaluate: () => CapabilityStatus.Unknown };
const report: CapabilityReport = machine.evaluateCapability(policy.id);
// @ts-expect-error A boolean is not a capability status.
const invalidPolicy: Capability<Phase, number | null> = { id: Symbol(), evaluate: () => false };
void [report, invalidPolicy];
