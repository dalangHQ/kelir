// see: cans/02-plugin-model.md#Foreign-plugin-proxy
// see: cans/08-lifecycle.md#Session-and-context-handles
import {
  FusedTrampoline,
  RingEvent,
  VetoEvent,
  type HookDescriptor,
  type LedgerHandle,
  type LifecycleContext,
} from "../dist/index.js";

function check(condition: boolean, label: string): void {
  if (!condition) {
    throw new Error(`fusion.trampoline.test: ${label}`);
  }
}

function fusedRunsKeepTheRingOrdinalPayloadForNonLifecycleHooks(): void {
  const seen: unknown[] = [];
  const hooks: readonly HookDescriptor[] = [
    {
      ring: RingEvent.Trap,
      stage: "pre",
      ordinal: 0,
      run: (event: unknown): unknown => seen.push(event),
    },
  ];
  const runs = FusedTrampoline.fuse(hooks).runs();
  runs[0].run(RingEvent.Trap, 0);
  const payload = seen[0] as { ring: RingEvent; ordinal: number };
  check(
    payload.ring === RingEvent.Trap && payload.ordinal === 0,
    "non-lifecycle hooks still receive the ring/ordinal payload",
  );
}

function fusedRunsDeliverALifecycleContextToLifecycleHooks(): void {
  const seen: unknown[] = [];
  const hooks: readonly HookDescriptor[] = [
    {
      ring: RingEvent.Lifecycle,
      stage: "single",
      ordinal: 0,
      lifecycleTag: "vm-ready",
      run: (event: unknown): unknown => seen.push(event),
    },
  ];
  const ledger = {
    flush: async (): Promise<void> => undefined,
  } as LedgerHandle;
  const runs = FusedTrampoline.fuse(hooks).runs({ ledger });
  runs[0].run(RingEvent.Lifecycle, 0);
  const ctx = seen[0] as LifecycleContext;
  check(
    typeof ctx.setWatchdog === "function" &&
      typeof ctx.registerVeto === "function",
    "lifecycle hooks receive a LifecycleContext with its control verbs",
  );
  check(
    ctx.cap?.ledger === ledger,
    "the lifecycle ctx carries the cap ledger handle",
  );
  ctx.setWatchdog({ timeoutMs: 10, action: "reboot" });
  ctx.registerVeto(VetoEvent.Suspend, () => true);
  const bare = FusedTrampoline.fuse(hooks).runs();
  bare[0].run(RingEvent.Lifecycle, 0);
  check(
    (seen[1] as LifecycleContext).cap === undefined,
    "runs without a cap deliver a ctx with no cap",
  );
}

async function main(): Promise<void> {
  fusedRunsKeepTheRingOrdinalPayloadForNonLifecycleHooks();
  fusedRunsDeliverALifecycleContextToLifecycleHooks();
}

main().catch((error: unknown): never => {
  throw error;
});
