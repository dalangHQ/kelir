// see: cans/03-sdk-architecture.md#TypeScript-SDK
import { kelir } from "./builder.lifecycle.js";
import type { ValidateDeps } from "./builder.lifecycle.js";
import { createPlugin } from "./plugin.model.js";
import { t } from "./schema.config.js";
import type { LedgerHandle } from "./context.lifecycle.types.js";
import { RingEvent } from "./fusion.plugin.js";

function check(condition: boolean, label: string): void {
  if (!condition) {
    throw new Error(`accumulation.monad.test: ${label}`);
  }
}

const secretMask = createPlugin("secretMask", { maskActive: true });
const secureWorkspace = createPlugin("secureWorkspace")
  .requires<typeof secretMask>()
  .derive((c) => ({ workspaceSecured: c.secretMask.maskActive }));

// see: cans/03-sdk-architecture.md#TypeScript-SDK
type MissingDepGate = ValidateDeps<unknown, typeof secureWorkspace> extends {
  readonly __missing_dependency: "secretMask";
} ? true : false;
const missingDepGate: MissingDepGate = true;
void missingDepGate;

const runtime = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(secretMask)
  .use(secureWorkspace)
  .params(t.Object({
    action: t.Literal("fix-webhook"),
    issueId: t.String(),
    tags: t.Array(t.String()),
  }))
  .trap("git", (c) => {
    const action: string = c.params.action;
    const tags: readonly string[] = c.params.tags;
    const masked: boolean = c.secretMask.maskActive;
    const secured: boolean = c.secureWorkspace.workspaceSecured;
    void action;
    void tags;
    void masked;
    void secured;
    return c.pass();
  });

const storageFirst = kelir({ vcpus: 4, memoryMb: 4096 })
  .storage<{ ledger: LedgerHandle }>({})
  .use(secretMask)
  .trap("git", (c) => {
    const ledger: LedgerHandle = c.ledger;
    const masked: boolean = c.secretMask.maskActive;
    void ledger;
    void masked;
    return c.pass();
  });

const streamed = kelir({ vcpus: 4, memoryMb: 4096 })
  .registerStreamKind(7, t.Object({ ping: t.String() }), (request) => ({ pong: request.ping }));

const vsock = kelir({ vcpus: 4, memoryMb: 4096 })
  .registerVsockService(1024, "ping", t.Object({ ping: t.String() }), (request) => ({ pong: request.ping }));

const denied = kelir({ vcpus: 4, memoryMb: 4096 })
  .use(secretMask)
  .onTrapDeny((event, session) => {
    const masked: boolean = session.secretMask.maskActive;
    void event;
    void masked;
  });

// see: cans/03-sdk-architecture.md#TypeScript-SDK
function accumulationMonadFuses(): void {
  check(runtime.fuse().hookCount(RingEvent.Trap) === 1, "positive chain fuses one trap hook");
  check(storageFirst.fuse().hookCount(RingEvent.Trap) === 1, "storage-first chain fuses one trap hook");
  check(streamed.fuse().hookCount(RingEvent.VsockService) === 1, "stream kind fuses one vsock hook");
  check(vsock.vsockServices.length === 1, "vsock service registers one entry");
  check(denied.fuse().hookCount(RingEvent.Lifecycle) === 1, "trap-deny fuses one lifecycle hook");
}

async function main(): Promise<void> {
  accumulationMonadFuses();
}

main().catch((error: unknown): never => {
  throw error;
});
