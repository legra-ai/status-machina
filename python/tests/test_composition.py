"""Kernel-decided composition between native machines: routing, binding and propagation."""
import unittest
from dataclasses import dataclass
from enum import Enum, IntEnum, auto

from status_machina import (
    Action,
    AddressMismatchError,
    AliasedBindingError,
    BatchLimitError,
    BindingMismatchError,
    BusyError,
    CallbackError,
    Capability,
    CapabilityReport,
    CapabilitySnapshotLimitError,
    CapabilityStatus,
    Composition,
    CompositionError,
    CompositionLimitError,
    Deferred,
    Definition,
    Delivery,
    DisposedError,
    DuplicateBindingError,
    DuplicateInstanceError,
    DuplicateObservationError,
    DuplicateRouteError,
    Effect,
    Event,
    ForeignSnapshotError,
    InstanceAddress,
    InstanceRegistry,
    InstanceRegistryLimitError,
    LimitError,
    MissingBindingError,
    OutputEvent,
    OutputIntent,
    PropagationLimits,
    PropagationLimitsError,
    PropagationStatus,
    ReportInstanceMismatchError,
    Route,
    Routed,
    State,
    Transition,
    Transitioned,
    UndeclaredCapabilityError,
    UndeclaredRouteEventError,
    Unmatched,
    UnregisteredBindingError,
    UnregisteredRouteAddressError,
    UnregisteredSourceError,
)


class Order(Enum):
    OPEN = auto()
    PAID = auto()
class OrderInput(Enum):
    PAY = auto()
class OrderOutput(Enum):
    PAID = auto()
    AUDITED = auto()
class Shipment(Enum):
    IDLE = auto()
    SHIPPING = auto()
class ShipmentInput(Enum):
    SHIP = auto()
class Policy(Enum):
    READY = auto()
class Tenant(IntEnum):
    A = 1
class Other(IntEnum):
    A = 1

@dataclass
class Payment:
    order: str
    amount: int

@dataclass
class Counter:
    shipped: int = 0

def refuse_on_facts(data, payload, facts):
    if facts == "fail":
        raise ValueError("carrier offline")
    data.shipped += 1

ORDERS = Definition(id="orders", initial=Order.OPEN, states=(State(Order.OPEN), State(Order.PAID)),
    transitions=(Transition(id="pay", source=Order.OPEN, event=OrderInput.PAY, target=Order.PAID,
        outputs=(OutputEvent(OrderOutput.PAID, describe=lambda data, payload, facts: payload),
                 OutputEvent(OrderOutput.AUDITED, describe=lambda data, payload, facts: payload.order))),))
SHIPPING = Definition(id="shipping", initial=Shipment.IDLE, states=(State(Shipment.IDLE), State(Shipment.SHIPPING)),
    transitions=(Transition(id="ship", source=Shipment.IDLE, event=ShipmentInput.SHIP, target=Shipment.SHIPPING,
        actions=(Action("count", apply=refuse_on_facts),),
        effects=(Effect("label", describe=lambda data, payload, facts: ("label", payload)),)),),
    capabilities=(Capability(Policy.READY, lambda state, data, facts:
        CapabilityStatus.AVAILABLE if facts else CapabilityStatus.UNAVAILABLE),))
LIMITS = PropagationLimits(max_steps=8, max_intents=8)

class Pipeline:
    """One order machine routed to one shipping machine in one tenant scope."""
    def __init__(self, scope=Tenant.A, capability=None):
        self.order = ORDERS.instantiate(id="order-1", data=None)
        self.shipping = SHIPPING.instantiate(id="shipment-1", data=Counter())
        self.source = InstanceAddress(scope, "order-1")
        self.destination = InstanceAddress(scope, "shipment-1")
        self.route = Route(self.source, OrderOutput.PAID, self.destination, ShipmentInput.SHIP, capability)
        self.composition = Composition(sources=InstanceRegistry(((self.source, self.order),)),
            destinations=InstanceRegistry(((self.destination, self.shipping),)), routes=(self.route,))

    def bind(self):
        return self.composition.bind({self.source: self.order, self.destination: self.shipping})

    def pay(self, amount=10):
        return Delivery(self.source, Event(OrderInput.PAY, Payment("order-1", amount)))

class PropagationTests(unittest.TestCase):
    def test_outputs_are_routed_fifo_with_native_payloads_and_addressed_effects(self):
        pipeline = Pipeline()
        with pipeline.bind() as bound:
            report = bound.propagate([pipeline.pay()], limits=LIMITS)
        self.assertIs(report.status, PropagationStatus.QUIESCENT)
        self.assertEqual([step.destination for step in report.steps], [pipeline.source, pipeline.destination])
        self.assertIsInstance(report.steps[1].outcome, Transitioned)
        self.assertEqual(report.steps[1].outcome.record.target, Shipment.SHIPPING)
        effect, = report.effects
        self.assertEqual(effect.source, pipeline.destination)
        self.assertEqual(effect.intent.payload, ("label", Payment("order-1", 10)))
        unmatched, = report.unrouted
        self.assertIsInstance(unmatched, Unmatched)
        self.assertEqual(unmatched.intent, OutputIntent(OrderOutput.AUDITED, "order-1"))
        self.assertEqual((pipeline.order.state, pipeline.shipping.data), (Order.PAID, Counter(1)))

    def test_later_failure_keeps_accepted_steps_and_resumes_from_pending(self):
        pipeline = Pipeline()
        with pipeline.bind() as bound:
            report = bound.propagate([pipeline.pay()], limits=LIMITS, facts="fail")
            self.assertIs(report.status, PropagationStatus.DISPATCH_FAILED)
            self.assertIsInstance(report.error, CallbackError)
            self.assertIsInstance(report.error.__cause__, ValueError)
            self.assertEqual(len(report.steps), 1)
            delivery, = report.pending
            self.assertEqual(delivery.destination, pipeline.destination)
            self.assertEqual(delivery.event, Event(ShipmentInput.SHIP, Payment("order-1", 10)))
            resumed = bound.propagate(report.pending, limits=LIMITS, facts="ok")
        self.assertIs(resumed.status, PropagationStatus.QUIESCENT)
        self.assertEqual((pipeline.order.revision, pipeline.shipping.revision), (1, 1))

    def test_budget_stops_return_original_inputs_and_unbound_destinations(self):
        pipeline = Pipeline()
        first, second = pipeline.pay(1), pipeline.pay(2)
        with pipeline.bind() as bound:
            report = bound.propagate([first, second], limits=PropagationLimits(8, 1))
            self.assertIs(report.status, PropagationStatus.INTENT_LIMIT)
            self.assertEqual(report.steps, ())
            self.assertIs(report.pending[0], first)
            report = bound.propagate([first, second], limits=PropagationLimits(1, 8))
            self.assertIs(report.status, PropagationStatus.STEP_LIMIT)
            self.assertIs(report.pending[0], second)
            self.assertEqual(report.pending[1].destination, pipeline.destination)
            stray = Delivery(InstanceAddress(Tenant.A, "unknown"), Event(OrderInput.PAY, None))
            report = bound.propagate([stray], limits=LIMITS)
            self.assertIs(report.status, PropagationStatus.UNBOUND_DESTINATION)
            self.assertIs(report.pending[0], stray)

    def test_scopes_isolate_equal_instance_identities(self):
        tenant, other = Pipeline(Tenant.A), Pipeline(Other.A)
        self.assertNotEqual(tenant.source, other.source)
        # Other.A == Tenant.A numerically, yet the scopes are distinct addresses.
        with self.assertRaises(UnregisteredBindingError):
            other.composition.bind({tenant.source: other.order, other.destination: other.shipping})
        with tenant.bind() as bound:
            report = bound.propagate([other.pay()], limits=LIMITS)
        self.assertIs(report.status, PropagationStatus.UNBOUND_DESTINATION)
        self.assertEqual((tenant.order.revision, other.order.revision), (0, 0))

class RoutingTests(unittest.TestCase):
    def test_route_batch_orders_items_and_defers_unobserved_capabilities(self):
        pipeline = Pipeline(capability=Policy.READY)
        outputs = (OutputIntent(OrderOutput.PAID, 7), OutputIntent(OrderOutput.AUDITED, 8))
        deferred, unmatched = pipeline.composition.route_batch(pipeline.source, outputs)
        self.assertIsInstance(deferred, Deferred)
        self.assertEqual((deferred.capability, deferred.status), (Policy.READY, CapabilityStatus.UNKNOWN))
        self.assertIs(deferred.intent, outputs[0])
        self.assertIs(unmatched.intent, outputs[1])
        report = pipeline.shipping.evaluate_capability(Policy.READY, facts=True)
        snapshot = pipeline.composition.capabilities(revisions=((pipeline.destination, 0),),
            reports=((pipeline.destination, report),))
        routed, _ = pipeline.composition.route_batch(pipeline.source, outputs, capabilities=snapshot)
        self.assertEqual(routed, Routed(pipeline.source, pipeline.destination, Event(ShipmentInput.SHIP, 7)))
        stale = pipeline.composition.capabilities(revisions=((pipeline.destination, 1),),
            reports=((pipeline.destination, report),))
        item, _ = pipeline.composition.route_batch(pipeline.source, outputs, capabilities=stale)
        self.assertEqual(item.status, CapabilityStatus.UNKNOWN)

    def test_propagation_defers_until_capability_is_observed(self):
        pipeline = Pipeline(capability=Policy.READY)
        report = pipeline.shipping.evaluate_capability(Policy.READY, facts=False)
        snapshot = pipeline.composition.capabilities(revisions=((pipeline.destination, 0),),
            reports=((pipeline.destination, report),))
        with pipeline.bind() as bound:
            result = bound.propagate([pipeline.pay()], limits=LIMITS, capabilities=snapshot)
        deferred = [item for item in result.unrouted if isinstance(item, Deferred)]
        self.assertEqual(deferred[0].status, CapabilityStatus.UNAVAILABLE)
        self.assertEqual(pipeline.shipping.revision, 0)

    def test_route_batch_rejections_keep_every_output(self):
        pipeline = Pipeline()
        outputs = [OutputIntent(OrderOutput.PAID, n) for n in range(3)]
        with self.assertRaises(UnregisteredSourceError) as caught:
            pipeline.composition.route_batch(pipeline.destination, outputs)
        self.assertEqual(caught.exception.outputs, tuple(outputs))
        small = Composition(sources=InstanceRegistry(((pipeline.source, pipeline.order),)),
            destinations=InstanceRegistry(((pipeline.destination, pipeline.shipping),)), max_batch=2)
        with self.assertRaises(BatchLimitError) as caught:
            small.route_batch(pipeline.source, iter(outputs))
        self.assertIsInstance(caught.exception, LimitError)
        self.assertEqual(caught.exception.context["limit"], 2)
        with self.assertRaises(TypeError):
            pipeline.composition.route_batch(pipeline.source, [(OrderOutput.PAID, 1)])

class RejectionTests(unittest.TestCase):
    def test_registry_rejections_name_entry_address_and_machine(self):
        pipeline = Pipeline()
        wrong = InstanceAddress(Tenant.A, "order-2")
        with self.assertRaises(AddressMismatchError) as caught:
            Composition(sources=InstanceRegistry(((wrong, pipeline.order),)), destinations=InstanceRegistry(()))
        self.assertEqual((caught.exception.context["registry"], caught.exception.context["entry"]), ("sources", 0))
        self.assertIs(caught.exception.context["machine"], pipeline.order)
        with self.assertRaises(DuplicateInstanceError):
            Composition(sources=InstanceRegistry(((pipeline.source, pipeline.order),) * 2),
                destinations=InstanceRegistry(()))
        for registry in (InstanceRegistry((), max_instances=0),
                InstanceRegistry(((pipeline.source, pipeline.order), (pipeline.destination, pipeline.shipping)), max_instances=1)):
            with self.assertRaises(InstanceRegistryLimitError):
                Composition(sources=registry, destinations=InstanceRegistry(()))

    def test_route_rejections_name_the_native_route(self):
        pipeline = Pipeline()
        sources = InstanceRegistry(((pipeline.source, pipeline.order),))
        destinations = InstanceRegistry(((pipeline.destination, pipeline.shipping),))
        cases = (
            (DuplicateRouteError, (pipeline.route, pipeline.route), 1),
            (UnregisteredRouteAddressError, (Route(pipeline.source, OrderOutput.PAID,
                InstanceAddress(Tenant.A, "ghost"), ShipmentInput.SHIP),), 0),
            (UndeclaredRouteEventError, (pipeline.route, Route(pipeline.source, OrderInput.PAY,
                pipeline.destination, ShipmentInput.SHIP)), 1),
        )
        for kind, routes, index in cases:
            with self.subTest(kind=kind.__name__), self.assertRaises(kind) as caught:
                Composition(sources=sources, destinations=destinations, routes=routes)
            self.assertIsInstance(caught.exception, CompositionError)
            self.assertIs(caught.exception.context["route"], routes[index])
        with self.assertRaises(CompositionLimitError):
            Composition(sources=sources, destinations=destinations, max_batch=0)
        with self.assertRaises(UndeclaredCapabilityError):
            Composition(sources=sources, destinations=destinations, routes=(Route(pipeline.source,
                OrderOutput.PAID, pipeline.destination, ShipmentInput.SHIP, "unknown"),))

    def test_binding_is_exclusive_complete_and_unaliased(self):
        pipeline = Pipeline()
        with self.assertRaises(MissingBindingError) as caught:
            pipeline.composition.bind({pipeline.source: pipeline.order})
        self.assertEqual(caught.exception.context["address"], pipeline.destination)
        with self.assertRaises(BindingMismatchError):
            pipeline.composition.bind({pipeline.source: pipeline.shipping, pipeline.destination: pipeline.order})
        with self.assertRaises(DuplicateBindingError):
            pipeline.composition.bind([(pipeline.source, pipeline.order),
                (pipeline.source, ORDERS.instantiate(id="order-1", data=None))])
        with self.assertRaises(AliasedBindingError):
            pipeline.composition.bind([(pipeline.source, pipeline.order), (pipeline.source, pipeline.order)])
        twin = InstanceAddress("audit", "order-1")
        both = Composition(sources=InstanceRegistry(((pipeline.source, pipeline.order), (twin, pipeline.order))),
            destinations=InstanceRegistry(()))
        with self.assertRaises(AliasedBindingError):
            both.bind({pipeline.source: pipeline.order, twin: pipeline.order})
        bound = pipeline.bind()
        with self.assertRaises(BusyError):
            pipeline.order.dispatch(Event(OrderInput.PAY, None))
        with self.assertRaises(BusyError):
            pipeline.shipping.dispose()
        with self.assertRaises(BusyError):
            pipeline.bind()
        bound.dispose()
        bound.dispose()
        with self.assertRaises(DisposedError):
            bound.propagate([], limits=LIMITS)
        self.assertEqual(pipeline.order.state, Order.OPEN)

    def test_reentrant_propagation_fails_the_step_without_acceptance(self):
        holder = {}
        def reenter(data, payload, facts):
            holder["bound"].propagate([], limits=LIMITS)
        looping = Definition(id="loop", initial=Order.OPEN, states=(State(Order.OPEN), State(Order.PAID)),
            transitions=(Transition(id="pay", source=Order.OPEN, event=OrderInput.PAY, target=Order.PAID,
                actions=(Action("reenter", apply=reenter),)),))
        machine = looping.instantiate(id="loop", data=None)
        address = InstanceAddress(Tenant.A, "loop")
        composition = Composition(sources=InstanceRegistry(()), destinations=InstanceRegistry(((address, machine),)))
        with composition.bind({address: machine}) as bound:
            holder["bound"] = bound
            report = bound.propagate([Delivery(address, Event(OrderInput.PAY, None))], limits=LIMITS)
        self.assertIsInstance(report.error.__cause__, BusyError)
        self.assertEqual(machine.revision, 0)

    def test_limits_and_snapshots_are_kernel_checked(self):
        for steps, intents in ((0, 1), (1, 4097), (-1, 1), (2**70, 1)):
            with self.subTest(steps=steps, intents=intents), self.assertRaises(PropagationLimitsError):
                PropagationLimits(steps, intents)
        with self.assertRaises(TypeError):
            PropagationLimits(True, 1)  # type: ignore[arg-type]
        pipeline, other = Pipeline(capability=Policy.READY), Pipeline()
        report = pipeline.shipping.evaluate_capability(Policy.READY, facts=True)
        composition = pipeline.composition
        with self.assertRaises(ReportInstanceMismatchError):
            composition.capabilities(reports=((pipeline.source, report),))
        with self.assertRaises(DuplicateObservationError):
            composition.capabilities(revisions=((pipeline.destination, 0),) * 2)
        with self.assertRaises(CapabilitySnapshotLimitError):
            composition.capabilities(revisions=((pipeline.source, 0), (pipeline.destination, 0)), max_observations=1)
        with self.assertRaises(TypeError):
            composition.capabilities(reports=((pipeline.destination,
                CapabilityReport(Policy.READY, "shipment-1", 0, CapabilityStatus.AVAILABLE)),))
        with self.assertRaises(ForeignSnapshotError):
            other.composition.route_batch(other.source, (), capabilities=composition.capabilities())

if __name__ == "__main__":
    unittest.main()
