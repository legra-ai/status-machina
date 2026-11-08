"""Capabilities observe native values without authorizing or mutating machines."""
from enum import Enum, auto
import unittest
from status_machina import (
    Behavior, BehaviorRef, BehaviorRegistry, BehaviorRole, CallbackError, Capability,
    CapabilityStatus, Definition, DefinitionError, State,
)

class Phase(Enum):
    OPEN = auto()
class Policy(Enum):
    EDIT = auto()
class Story(Enum):
    EDITABLE = auto()

class CapabilityTests(unittest.TestCase):
    def definition(self, evaluator, *, behaviors=BehaviorRegistry()):
        return Definition(id=object(), initial=Phase.OPEN, states=(State(Phase.OPEN),),
            transitions=(), capabilities=(Capability(Policy.EDIT, evaluator),), behaviors=behaviors)

    def test_native_report_is_advisory_and_detached(self):
        def evaluate(state, data, facts):
            self.assertIs(state, Phase.OPEN)
            data.append(99)
            facts.append(99)
            return CapabilityStatus.AVAILABLE
        facts = [1]
        identity = object()
        with self.definition(evaluate).instantiate(id=identity, data=[1]) as machine:
            report = machine.evaluate_capability(Policy.EDIT, facts=facts)
            self.assertIs(report.capability, Policy.EDIT)
            self.assertIs(report.instance, identity)
            self.assertIs(report.status, CapabilityStatus.AVAILABLE)
            self.assertEqual(report.revision, 0)
            self.assertEqual(machine.data, [1])
            self.assertEqual(facts, [1])
            self.assertEqual(machine.revision, 0)

    def test_story_binding_uses_exact_native_identity(self):
        reference = BehaviorRef(Story.EDITABLE, object())
        behaviors = BehaviorRegistry((Behavior(reference, BehaviorRole.CAPABILITY,
            lambda state, data, facts: CapabilityStatus.UNKNOWN),))
        with self.definition(reference, behaviors=behaviors).instantiate(id=object(), data=None) as machine:
            self.assertIs(machine.evaluate_capability(Policy.EDIT, facts=None).status, CapabilityStatus.UNKNOWN)
        with self.assertRaises(DefinitionError):
            self.definition(BehaviorRef(Story.EDITABLE, object()), behaviors=behaviors)

    def test_failure_is_not_unavailable_and_retains_cause(self):
        failure = ValueError('policy failed')
        def evaluate(*args):
            raise failure
        with self.definition(evaluate).instantiate(id=object(), data=None) as machine:
            with self.assertRaises(CallbackError) as caught:
                machine.evaluate_capability(Policy.EDIT, facts=None)
            self.assertIs(caught.exception.__cause__, failure)
            self.assertEqual(caught.exception.context['phase'], 'capability')
            self.assertEqual(machine.revision, 0)

    def test_boolean_status_unknown_policy_and_duplicates_fail(self):
        with self.definition(lambda *args: False).instantiate(id=object(), data=None) as machine:
            with self.assertRaises(CallbackError):
                machine.evaluate_capability(Policy.EDIT, facts=None)
            with self.assertRaises(DefinitionError):
                machine.evaluate_capability(object(), facts=None)
        with self.assertRaises(DefinitionError):
            Definition(id=object(), initial=Phase.OPEN, states=(State(Phase.OPEN),), transitions=(),
                capabilities=(Capability(Policy.EDIT, lambda *a: CapabilityStatus.AVAILABLE),
                    Capability(Policy.EDIT, lambda *a: CapabilityStatus.UNAVAILABLE)))
