"""Native graph inspection uses explicit labels and never evaluates behaviors."""
from enum import Enum, auto
import unittest
from status_machina import (
    Condition, Definition, DiagramLabels, Disposition, InspectionError,
    InspectionLimits, State, Transition,
)

class Phase(Enum):
    READY = auto()
    DONE = auto()
class Input(Enum):
    FINISH = auto()
    DUPLICATE = auto()

def forbidden(*args):
    raise AssertionError('inspection evaluated application behavior')

class InspectionTests(unittest.TestCase):
    def setUp(self):
        self.edge = object()
        self.guard = object()
        self.rule = object()
        self.definition = Definition(id=object(), initial=Phase.READY,
            states=(State(Phase.READY, invariant=forbidden), State(Phase.DONE, terminal=True)),
            transitions=(Transition(id=self.edge, source=Phase.READY, event=Input.FINISH,
                target=Phase.DONE, guard=Condition(self.guard, predicate=forbidden)),),
            dispositions=(Disposition(self.rule, Phase.DONE, Input.DUPLICATE,
                Condition(self.guard, predicate=forbidden), 'Already done'),))
        self.labels = DiagramLabels(states={Phase.READY: 'Same "label"', Phase.DONE: 'Same "label"'},
            transitions={self.edge: 'Finish <now>'}, events={Input.FINISH: 'Finish', Input.DUPLICATE: 'Duplicate'},
            conditions={self.guard: 'Allowed?'}, dispositions={self.rule: 'Harmless'})

    def test_coverage_retains_native_ids_and_structural_counts(self):
        coverage = self.definition.structural_coverage()
        self.assertEqual(tuple(row.id for row in coverage.states), (Phase.READY, Phase.DONE))
        self.assertTrue(coverage.states[0].initial)
        self.assertEqual(coverage.states[0].outgoing, 1)
        self.assertTrue(coverage.states[1].terminal)
        self.assertEqual(coverage.states[1].incoming, 1)
        self.assertEqual(coverage.states[1].dispositions, 1)
        self.assertEqual(coverage.transitions, (self.edge,))
        self.assertEqual(coverage.dispositions, (self.rule,))

    def test_mermaid_escapes_labels_without_merging_equal_labels(self):
        diagram = self.definition.diagram(self.labels)
        self.assertTrue(diagram.startswith('stateDiagram-v2\n'))
        self.assertEqual(diagram.count('state "Same #34;label#34;"'), 2)
        self.assertIn('Finish #60;now#62; / Finish [guard: Allowed#63;]', diagram)
        self.assertIn('disposition: Harmless / Duplicate', diagram)
        self.assertIn('s0 --> s1', diagram)
        self.assertEqual(diagram, self.definition.diagram(self.labels))

    def test_entry_output_and_label_limits_fail_explicitly(self):
        with self.assertRaises(InspectionError) as caught:
            self.definition.structural_coverage(limits=InspectionLimits(max_entries=3))
        self.assertEqual(caught.exception.context['limit'], 3)
        with self.assertRaises(InspectionError):
            self.definition.diagram(self.labels, limits=InspectionLimits(max_output_bytes=10))
        with self.assertRaises(InspectionError) as caught:
            self.definition.diagram(DiagramLabels())
        self.assertIs(caught.exception.context['identifier'], Phase.READY)
        with self.assertRaises(InspectionError):
            InspectionLimits(max_entries=0)
        with self.assertRaises(InspectionError):
            InspectionLimits(max_output_bytes=True)

    def test_invalid_label_is_not_stringified_and_utf8_limit_is_in_bytes(self):
        with self.assertRaises(InspectionError):
            self.definition.diagram(DiagramLabels(states={Phase.READY: object()}))
        with self.assertRaises(InspectionError):
            self.definition.diagram(DiagramLabels(states={Phase.READY: '\ud800'}))
        with self.assertRaises(InspectionError):
            self.definition.diagram(DiagramLabels(states={Phase.READY: '界' * 10}),
                limits=InspectionLimits(max_output_bytes=20))

    def test_state_report_and_diagram_follow_declaration_order_not_initial_first(self):
        graph = Definition(id=object(), initial=Phase.READY,
            states=(State(Phase.DONE, terminal=True), State(Phase.READY)),
            transitions=(Transition(id=self.edge, source=Phase.READY, event=Input.FINISH, target=Phase.DONE),))
        self.assertEqual(tuple(row.id for row in graph.structural_coverage().states), (Phase.DONE, Phase.READY))
        diagram = graph.diagram(DiagramLabels(states={Phase.DONE: 'Done', Phase.READY: 'Ready'},
            transitions={self.edge: 'Finish'}, events={Input.FINISH: 'Finish'}))
        self.assertIn('state "Done" as s0', diagram)
        self.assertIn('[*] --> s1', diagram)
