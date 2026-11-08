# see: cans/02-plugin-model.md#Segregated-ring-traits
# see: cans/02-plugin-model.md#Foreign-plugin-proxy
# see: cans/15-operations.md#Multi-language-parity-gate
from kelir.generated.fusion_tables import (
    HOOK_RINGS,
    LIFECYCLE_TAGS,
    HookStage,
    LifecycleTag,
    RingEvent,
)
from kelir.generated.protocol import Channel, ExecStream, MsgType, StreamKind
from kelir.generated.telemetry import (
    GuestClock,
    SnapshotDescriptor,
    ReceiptExtension,
    PhaseTimings,
    ExitKind,
    ExecutionReceipt,
    CausalDelta,
)
