"""Native exceptions with structured domain context and original callback causes."""
from types import MappingProxyType


class StatusMachinaError(Exception):
    """Base error; ``context`` retains native identifiers instead of wire labels."""
    def __init__(self, message: str, **context: object) -> None:
        super().__init__(message)
        self.context = MappingProxyType(context)
        self.phase = context.get("phase")

class DefinitionError(StatusMachinaError):
    """The proposed graph or binding is invalid."""
class DispatchError(StatusMachinaError):
    """Evaluation or checked acceptance failed without committing the decision."""
class CallbackError(DispatchError):
    """Application code raised an exception; ``__cause__`` is the original object."""
class SnapshotError(CallbackError):
    """Application data could not be isolated."""
class InvariantError(DispatchError):
    """Candidate data violates a declared state invariant."""
class PostconditionError(DispatchError):
    """Candidate data violates a transition postcondition."""
class AmbiguousTransitionError(DispatchError):
    """Multiple guarded transitions or dispositions matched."""
class ActionError(DispatchError):
    """A declared candidate action refused its operation."""
class StaleDecisionError(DispatchError):
    """The expected revision is no longer current."""
class ForeignDecisionError(DispatchError):
    """A decision belongs to another definition or instance incarnation."""
class RevisionError(DispatchError):
    """The monotonic revision cannot advance."""
class DisposedError(StatusMachinaError):
    """A disposed or consumed resource cannot be used."""
class BusyError(StatusMachinaError):
    """An operation is already active; recursive/concurrent mutation is refused."""
class LimitError(StatusMachinaError):
    """A declared resource bound was reached."""
class InspectionError(StatusMachinaError):
    """The requested inspection exceeds its bounds or has invalid labels."""

# Checked construction: ``context['reason']`` is the kernel variant and the context
# names the rejected element (``state``, ``transition``, ``condition``, ...) natively.
class UnknownStateError(DefinitionError):
    """The initial state, a transition endpoint or a disposition source is not declared."""
class DuplicateIdentifierError(DefinitionError):
    """Two states, transitions, dispositions or capabilities share one identity."""
class UnreachableStateError(DefinitionError):
    """``context['state']`` cannot be reached from the initial state."""
class TerminalTransitionError(DefinitionError):
    """The terminal ``context['state']`` declares an outgoing transition."""
class InternalTransitionError(DefinitionError):
    """An internal transition must leave and re-enter no state: its target is its source."""
class InvalidConditionError(DefinitionError):
    """A condition expression is malformed: an empty group, a missing operand or too deep."""
class InvalidDispositionError(DefinitionError):
    """A disposition reason must hold 1 to 256 non-whitespace bytes."""
class DefinitionLimitError(DefinitionError, LimitError):
    """A definition, state, transition or Story registry bound was exceeded or is invalid."""
class InitialInvariantError(DefinitionError):
    """Initial data violates the initial state's invariant; no instance was admitted."""
class UnknownCapabilityError(DefinitionError, LookupError):
    """The definition declares no ``context['capability']``."""

class RecordError(DefinitionError):
    """A portable definition record failed checked binding; ``context`` names native IDs."""
class ConditionRecordError(RecordError):
    """A condition record is structurally invalid; ``context['condition']`` is its ID."""
class TransitionRecordError(RecordError):
    """A transition record is structurally invalid; ``context['transition']`` is its ID."""
class DispositionRecordError(RecordError):
    """A disposition record is structurally invalid; ``context['disposition']`` is its ID."""
class BindingError(DefinitionError):
    """Exact Story resolution failed, for a specification, a record or an assertion; no
    implementation was substituted or loaded. ``context['reason']`` is the kernel variant."""
class UnknownBehaviorError(BindingError):
    """No implementation of ``context['story']`` is registered."""
class UnknownImplementationError(BindingError):
    """The Story is registered, but not ``context['implementation']``."""
class RoleMismatchError(BindingError):
    """The selected implementation has the ``context['actual']`` BehaviorRole, not ``expected``."""
class DuplicateImplementationError(BindingError):
    """A registry declares one Story and implementation pair twice."""
class IdentityMapError(StatusMachinaError):
    """An identity codec is invalid or was asked for an unregistered value."""
class IdentityRoleError(IdentityMapError):
    """A codec was supplied for, or requested from, the wrong identity role."""
class IdentityMapLimitError(IdentityMapError, LimitError):
    """The codec bound is invalid (1..4096) or the entries exceed it."""
class DuplicateIdentityError(IdentityMapError):
    """One identity occurs twice; ``context['entry']`` is the second position."""
class DuplicateRepresentationError(IdentityMapError):
    """Two identities share one external representation."""
class UnknownIdentityError(IdentityMapError, LookupError):
    """The identity has no registered representation; none is derived from str/repr."""
class UnknownRepresentationError(IdentityMapError, LookupError):
    """The representation has no registered identity; no default is substituted."""

class CompositionError(StatusMachinaError):
    """A composition was rejected by the kernel; ``context['reason']`` is the kernel variant."""
class InstanceRegistryError(CompositionError):
    """An InstanceRegistry entry was rejected; ``context`` names the registry, entry and address."""
class InstanceRegistryLimitError(InstanceRegistryError, LimitError):
    """The registry bound is invalid (1..512) or an instance/event inventory bound was reached."""
class AddressMismatchError(InstanceRegistryError):
    """The address does not name the supplied machine's identity."""
class DuplicateInstanceError(InstanceRegistryError):
    """The machine is already registered at this address."""
class CompositionDefinitionError(CompositionError):
    """A route or composition limit was rejected; ``context['route']`` is the native Route."""
class CompositionLimitError(CompositionDefinitionError, LimitError):
    """The route (1..512) or batch (1..256) bound is invalid, or the routes exceed it."""
class DuplicateRouteError(CompositionDefinitionError):
    """Two routes share one source address and output event."""
class UnregisteredRouteAddressError(CompositionDefinitionError):
    """A route endpoint is not in its registry; ``reason`` says which endpoint."""
class UndeclaredRouteEventError(CompositionDefinitionError):
    """The source never emits the routed event, or the destination never accepts its input event."""
class UndeclaredCapabilityError(CompositionDefinitionError):
    """A route requires a capability absent from the destination definition's inventory."""
class RouteBatchError(CompositionError):
    """A batch was refused before routing; ``outputs`` holds every intent, unconsumed."""
    def __init__(self, message: str, **context: object) -> None:
        super().__init__(message, **context)
        self.outputs = context.get("outputs", ())
class UnregisteredSourceError(RouteBatchError):
    """The batch source address is not registered as a source."""
class BatchLimitError(RouteBatchError, LimitError):
    """The batch holds more outputs than the composition's ``max_batch``."""
class CompositionBindingError(CompositionError):
    """Exclusive binding was refused before any machine was dispatched."""
class DuplicateBindingError(CompositionBindingError):
    """One address was bound twice."""
class UnregisteredBindingError(CompositionBindingError):
    """The bound address is in neither registry."""
class BindingMismatchError(CompositionBindingError):
    """The bound machine's identity or event signature differs from its registration."""
class MissingBindingError(CompositionBindingError):
    """A registered address has no binding; ``context['address']`` is the first one."""
class AliasedBindingError(CompositionBindingError):
    """One machine was bound at two addresses; exclusive use cannot alias."""
class PropagationLimitsError(CompositionError, LimitError):
    """Both propagation budgets must be explicit integers within 1..4096."""
class CapabilitySnapshotError(CompositionError):
    """A capability snapshot was rejected; ``context`` names the list, entry and address."""
class CapabilitySnapshotLimitError(CapabilitySnapshotError, LimitError):
    """The observation bound is invalid (1..1024) or the observations exceed it."""
class ReportInstanceMismatchError(CapabilitySnapshotError):
    """The report observed another instance than the address names."""
class DuplicateObservationError(CapabilitySnapshotError):
    """An address has two revisions, or two reports of one capability."""
class ForeignSnapshotError(CapabilitySnapshotError):
    """The snapshot was built by another composition and its address table."""

class RetryError(StatusMachinaError):
    """A retry policy or wait set was rejected; ``context['reason']`` is the kernel variant."""
class RetryPolicyError(RetryError):
    """The kernel refused the retry policy (``RetryPolicyBuildError``)."""
class ZeroDelayError(RetryPolicyError):
    """A fixed delay or initial backoff delay must be greater than zero."""
class TooFewAttemptsError(RetryPolicyError):
    """``max_attempts`` must include the first attempt and at least one retry (>= 2)."""
class CapBelowInitialError(RetryPolicyError):
    """The exponential ``maximum`` is shorter than its ``initial`` delay."""
class RetryWaitError(RetryError):
    """The kernel refused a wait; the wait set is unchanged (``RetryWaitError``)."""
class InvalidWaitCapacityError(RetryWaitError, LimitError):
    """The wait capacity must be an explicit integer within 1..4096."""
class WaitCapacityReachedError(RetryWaitError, LimitError):
    """Every active-wait slot is occupied; ``context['limit']`` is the capacity."""
class WaitGenerationExhaustedError(RetryWaitError):
    """The wait set cannot assign another never-reused generation."""
class RetryRangeError(RetryError, ValueError):
    """A value lies outside its kernel type: a negative duration, an attempt count outside
    0..2**32-1, a generation outside 0..2**64-1 or an instant outside the platform's
    ``SystemTime``. ``context['field']`` names it and ``context['value']`` holds it."""

class ScenarioError(StatusMachinaError):
    """Scenario replay was rejected by the kernel; ``context['reason']`` is the kernel variant."""
class ScenarioLimitsError(ScenarioError, LimitError):
    """A replay limit is zero or above its kernel ceiling (``ScenarioLimitsBuildError``)."""
class ScenarioSetError(ScenarioError):
    """The kernel refused the scenario set (``ScenarioSetBuildError``); ``context['entry']`` is the
    refused scenario's position and ``context['scenario']`` its identity."""
class EmptyScenarioSetError(ScenarioSetError):
    """A scenario set needs at least one scenario."""
class ScenarioCountLimitError(ScenarioSetError, LimitError):
    """More scenarios than ``max_scenarios``; ``context['limit']`` is the bound."""
class ScenarioStepLimitError(ScenarioSetError, LimitError):
    """A scenario has more steps than ``max_steps_per_scenario``."""
class ScenarioNodeLimitError(ScenarioSetError, LimitError):
    """The scenarios together have more steps than ``max_nodes``."""
class ScenarioAssertionError(ScenarioError):
    """The kernel refused a safety assertion (``ScenarioAssertionBuildError``)."""
class MissingAssertionPredicateError(ScenarioAssertionError):
    """A safety assertion needs a predicate or an exact ``BehaviorRef``."""
class ScenarioReplayError(ScenarioError):
    """The kernel refused the replay configuration (``ScenarioReplayBuildError``)."""
class AssertionLimitError(ScenarioReplayError, LimitError):
    """More safety assertions than the kernel's ceiling of 128."""
