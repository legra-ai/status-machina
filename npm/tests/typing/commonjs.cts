import machines = require('@legra-ai/status-machina');
const definition = new machines.Definition<string, string, number>({
  id: 'definition', initial: 'ready', states: [{ id: 'ready' }], transitions: [],
});
const machine = definition.instantiate({ id: 'instance', data: 1 });
const data: number = machine.data;
const revision: bigint = machine.revision;
// @ts-expect-error The CommonJS entry point retains data validation.
definition.instantiate({ id: 'invalid', data: 'wrong' });
void [data, revision];
