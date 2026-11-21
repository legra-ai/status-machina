import {
  BehaviorRef, BehaviorRegistry, BehaviorUse, DefinitionRecord, ForbiddenEdge, IdentityMap, IdentityRole, MachineIdentityMaps,
  NotRecord, PredicateRecord, StateRecord, TransitionDefinitionRecord, type ConditionRecord, type Definition,
} from '@legra-ai/status-machina';

enum Phase { Open, Done }
enum Input { Finish }
enum Wrong { Other }
const reference = new BehaviorRef(Symbol('story'), Symbol('implementation'));
const guard: ConditionRecord = new NotRecord(Symbol(), new PredicateRecord(new BehaviorUse(Symbol(), reference)));
if (guard.kind === 'not') { const operand: ConditionRecord = guard.operand; void operand; }
const record = new DefinitionRecord<Phase, Input>({
  id: Symbol(), initial: Phase.Open,
  states: [new StateRecord({ id: Phase.Open }), new StateRecord({ id: Phase.Done, terminal: true })],
  transitions: [new TransitionDefinitionRecord({ id: Symbol(), source: Phase.Open, event: Input.Finish, target: Phase.Done, guard })],
});
const definition: Definition<Phase, Input, number> = record.bind<number>(new BehaviorRegistry(), { snapshot: value => value });
const initial: Phase = definition.initial;
void initial;
// @ts-expect-error The initial state keeps the record's state type.
new DefinitionRecord<Phase, Input>({ id: Symbol(), initial: Wrong.Other, states: [], transitions: [] });
// @ts-expect-error Records select behaviors by BehaviorRef, never by function.
new StateRecord({ id: Phase.Open, invariant: () => true });
// @ts-expect-error A use site selects a BehaviorRef.
new BehaviorUse(Symbol(), () => undefined);
// @ts-expect-error Binding needs an explicit BehaviorRegistry.
record.bind([]);
const forbidding = new DefinitionRecord<Phase, Input>({ id: Symbol(), initial: Phase.Open, states: [], transitions: [],
  forbiddenEdges: [new ForbiddenEdge(Phase.Done, Phase.Open)] });
const forbiddenSource: Phase = forbidding.forbiddenEdges[0].source;
const boundEdges: readonly ForbiddenEdge<Phase>[] = forbidding.bind(new BehaviorRegistry()).forbiddenEdges;
void forbiddenSource; void boundEdges;
// @ts-expect-error A prohibition keeps the record's state type.
new DefinitionRecord<Phase, Input>({ id: Symbol(), initial: Phase.Open, states: [], transitions: [], forbiddenEdges: [new ForbiddenEdge(Wrong.Other, Wrong.Other)] });
// @ts-expect-error Records take ForbiddenEdge values, not plain pairs.
new DefinitionRecord<Phase, Input>({ id: Symbol(), initial: Phase.Open, states: [], transitions: [], forbiddenEdges: [{ source: Phase.Done, target: Phase.Open }] });

const states = new IdentityMap({ role: IdentityRole.State, entries: [[Phase.Open, 'ex:Open'] as const] });
const events = new IdentityMap({ role: IdentityRole.Event, entries: [[Input.Finish, 'ex:finish'] as const] });
const decoded: Phase = states.decode('ex:Open');
const encoded: 'ex:Open' = states.encode(Phase.Open);
void [decoded, encoded];
// @ts-expect-error The state codec only encodes its declared identity type.
states.encode(Wrong.Other);
const maps = new MachineIdentityMaps({ state: states, event: events });
const phase: Phase = maps.decode(IdentityRole.State, 'ex:Open');
const kind: Input = maps.decode('event', 'ex:finish');
void [phase, kind];
// @ts-expect-error The transition role has no configured codec.
maps.decode(IdentityRole.Transition, 'ex:t1');
// @ts-expect-error A state codec cannot fill the event role.
new MachineIdentityMaps({ event: states });
// @ts-expect-error Roles are IdentityRole values, not arbitrary strings.
new IdentityMap({ role: 'states', entries: [] });
