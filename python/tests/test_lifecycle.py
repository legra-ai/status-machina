"""Lifecycle, callback ownership and resource bounds use native Python behavior."""
from enum import Enum, auto
import gc
import threading
import unittest
import weakref
from status_machina import (Action, BusyError, CallbackError, Definition, DefinitionError,
    DisposedError, Effect, Event, ForeignDecisionError, LimitError, Limits, SnapshotError,
    StaleDecisionError, State, Transition)

class Phase(Enum):
    READY = auto()
    DONE = auto()
class Input(Enum):
    GO = auto()

def definition(*, actions=(), effects=(), limits=Limits(), snapshot=None):
    kwargs = {} if snapshot is None else {"snapshot": snapshot}
    return Definition(id=object(), initial=Phase.READY, limits=limits,
        states=(State(Phase.READY), State(Phase.DONE)),
        transitions=(Transition(id=object(), source=Phase.READY, event=Input.GO,
            target=Phase.DONE, actions=actions, effects=effects),), **kwargs)

class LifecycleTests(unittest.TestCase):
    def test_functional_action_can_replace_immutable_data_with_none(self):
        graph = definition(actions=(Action(id=object(), update=lambda data, event, facts: None),))
        with graph.instantiate(id=object(), data=7) as machine:
            machine.dispatch(Event(Input.GO, None))
            self.assertIsNone(machine.data)

    def test_decision_acceptance_is_single_use_and_checks_revision(self):
        with definition().instantiate(id=object(), data=7) as machine:
            first = machine.evaluate(Event(Input.GO, None))
            stale = machine.evaluate(Event(Input.GO, None))
            self.assertIs(machine.state, Phase.READY)
            self.assertEqual(first.expected_revision, 0)
            self.assertEqual(machine.accept(first).kind, "transitioned")
            with self.assertRaises(DisposedError):
                machine.accept(first)
            with self.assertRaises(StaleDecisionError):
                machine.accept(stale)
            self.assertEqual(machine.revision, 1)
            with self.assertRaises(DisposedError):
                _ = stale.expected_revision

    def test_foreign_instance_decision_cannot_be_accepted(self):
        graph = definition()
        with graph.instantiate(id=object(), data=7) as a, graph.instantiate(id=object(), data=7) as b:
            decision = a.evaluate(Event(Input.GO, None))
            with self.assertRaises(ForeignDecisionError):
                b.accept(decision)
            self.assertEqual(a.revision, 0)
            self.assertEqual(b.revision, 0)

    def test_reentrant_disposal_is_refused_and_outer_dispatch_is_unchanged(self):
        def action(*_):
            machine.dispose()
        with definition(actions=(Action(id=object(), apply=action),)).instantiate(id=object(), data=7) as machine:
            with self.assertRaises(CallbackError) as caught:
                machine.dispatch(Event(Input.GO, None))
            self.assertIsInstance(caught.exception.__cause__, BusyError)
            self.assertEqual(machine.revision, 0)

    def test_serialized_calls_can_run_on_another_python_thread(self):
        with definition().instantiate(id=object(), data=7) as machine:
            errors = []
            def drive():
                try:
                    machine.dispatch(Event(Input.GO, None))
                except BaseException as error:
                    errors.append(error)
            thread = threading.Thread(target=drive)
            thread.start()
            thread.join(timeout=5)
            self.assertFalse(thread.is_alive())
            self.assertEqual(errors, [])
            self.assertEqual(machine.revision, 1)

    def test_failed_effect_keeps_native_exception_and_stops_later_constructors(self):
        failure = ValueError("intent failed")
        calls = []
        def fails(*_):
            raise failure
        graph = definition(effects=(Effect(id=object(), describe=fails),
            Effect(id=object(), describe=lambda *_: calls.append(True))))
        with graph.instantiate(id=object(), data=7) as machine:
            with self.assertRaises(CallbackError) as caught:
                machine.dispatch(Event(Input.GO, None))
            self.assertIs(caught.exception.__cause__, failure)
            self.assertEqual(calls, [])
            self.assertEqual(machine.revision, 0)

    def test_domain_data_snapshot_is_not_applied_to_event_or_facts(self):
        def snapshot_data(data):
            return {"items": list(data["items"])}
        graph = definition(snapshot=snapshot_data,
            actions=(Action(id=object(), apply=lambda data, event, facts: data["items"].append(event)),))
        with graph.instantiate(id=object(), data={"items": []}) as machine:
            machine.dispatch(Event(Input.GO, 7), facts=None)
            self.assertEqual(machine.data, {"items": [7]})

    def test_snapshot_exception_is_native_and_never_admits_initial_data(self):
        failure = ValueError("cannot snapshot")
        def snapshot(data):
            raise failure
        with self.assertRaises(SnapshotError) as caught:
            definition(snapshot=snapshot).instantiate(id=object(), data=7)
        self.assertIs(caught.exception.__cause__, failure)

    def test_async_callbacks_are_rejected_at_construction(self):
        async def action(*_):
            pass
        with self.assertRaises(DefinitionError):
            definition(actions=(Action(id=object(), apply=action),))

    def test_callback_machine_cycle_is_visible_to_python_gc(self):
        class Handler:
            machine = None
            def __call__(self, *_):
                pass
        handler = Handler()
        graph = definition(actions=(Action(id=object(), apply=handler),))
        machine = graph.instantiate(id=object(), data=7)
        handler.machine = machine
        reference = weakref.ref(handler)
        del graph, machine, handler
        gc.collect()
        self.assertIsNone(reference())

    def test_disposal_releases_callback_owner_when_no_definition_owner_remains(self):
        class Handler:
            def __call__(self, *_):
                pass
        handler = Handler()
        graph = definition(actions=(Action(id=object(), apply=handler),))
        machine = graph.instantiate(id=object(), data=7)
        reference = weakref.ref(handler)
        del graph, handler
        machine.dispose()
        gc.collect()
        self.assertIsNone(reference())

    def test_state_action_iterable_stops_at_its_bound(self):
        consumed = []
        def actions():
            for index in range(100):
                consumed.append(index)
                yield Action(id=index, apply=lambda *_: None)
        with self.assertRaises(LimitError):
            State(Phase.READY, entry_actions=actions())
        self.assertEqual(len(consumed), 17)

if __name__ == "__main__":
    unittest.main()
