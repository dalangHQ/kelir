// ============================================================================
// serverless-webhook.ts
// ============================================================================
//
// Scale-to-zero webhook handler. Inbound TCP parks at the host proxy during
// suspend; the first post-wake frame splices to vsock Channel 1. While
// awaiting the next event, the guest memory ceiling is shrunk with the
// standard c.vm.adjust_balloon() API so host RSS drops. MCP clients attach
// on a dynamic console port without colliding with the PTY port.
//
// Mounts:     wire-ingress, mcp, tool-guard, ledger.
// Primitives: adjust_balloon.

import { kelir, t,
         type ExecutionReceipt,
         type PostExecAction } from "@kelir/core";
import { wireIngress } from "@kelir/plugin-wire-ingress";
import { mcp }         from "@kelir/plugin-mcp";
import { toolGuard }   from "@kelir/plugin-tool-guard";
import { ledger }      from "@kelir/plugin-ledger";

interface WebhookRequest {
  id: string;
  headers: Record<string, string>;
  body: Uint8Array;
}

interface WebhookResponse {
  status: number;
  headers: Record<string, string>;
  body: Uint8Array;
}

declare const handleWebhook: (req: WebhookRequest) => Promise<WebhookResponse>;

const runtime = kelir({
  vcpus: 2, memoryMb: 512, driver: "auto",
  kernel: "./fixtures/vmlinux-6.6-arm64-kelir",
  rootfsErofs: "./fixtures/rootfs-webhook.erofs",
  tscLocked: true, enableWatchdog: true,
})

  // → c.ingress
  .use(wireIngress({
    // Park up to 64 KiB per inbound connection during suspend; the TCP
    // handshake completes host-side and the frames queue without dropping.
    listen: { port: 8443, protocol: "https" },
    parking: { maxBufferedBytes: 64 * 1024, idleTimeoutMs: 30_000 },
    // First post-wake frame splices to vsock Channel 1. The host proxy
    // has already parsed routing metadata with zero allocation.
    wakeOnPacket: { vsockChannel: 1 },
    // Burst absorption: queue up to 256 inbound connections across the
    // suspend→resume window; overflow returns 503 without touching RAM.
    backlog: { maxConnections: 256, overflow: "reject-503" },
  }))

  // Idle memory: the standard balloon API shrinks the ceiling while
  // parked (see the vm:ready handler below); the trap pre-handler
  // grows it back before the webhook handler runs. No separate
  // compaction plugin is mounted.

  // → c.mcp
  .use(mcp({
    // A dynamic console port carries JSON-RPC 2.0 stdio directly to the MCP client. No
    // PTY conflict (interactive shell on a separate port).
    transport: { port: "dynamic", framing: "json-rpc-2.0-stdio" },
    advertise: { tools: ["webhook.dispatch", "webhook.replay"] },
  }))

  // → c.tool-guard (upstream of mcp on the console port)
  .use(toolGuard({
    // Anti-rug-pull: BLAKE3 over all tool schemas locks at on_vm_ready.
    // Any post-boot schema mutation halts with ESecurityViolation.
    pinSchemas: true,
    // Per-tool cadence ceiling; refuses ECYCLICREWRITE past this rate.
    maxInvocationsPerTrace: 50,
    approvals: {
      "webhook.replay": { action: "hold", durationMs: 500 },
    },
  }))
  // → c.ledger
  .use(ledger({
    store: "sqlite://.kelir/webhook.merkle.db",
    merkleChain: true,
    otel: { traceParent: true, serviceName: "kelir-webhook" },
  }))

  .params(t.Object({
    action: t.Literal("dispatch-webhook"),
    webhookId: t.String(),
  }))
  .trap("dispatch-webhook", {
    // First post-wake frame: raise the RAM ceiling back to 512 MB before
    // the handler runs. The idle balloon shrink had reduced it.
    pre: async (c) => {
      await c.vm.adjust_balloon(512);
      return c.resume();
    },
    post: async (c) => {
      const receipt: ExecutionReceipt = await c.receipt();
      // Zero dirty bytes means the handler short-circuited; the body
      // cannot be fresh. Rollback rather than serve a stale response.
      if (receipt.dirtyBytes === 0) return PostExecAction.RollbackAndHalt;
      return PostExecAction.Proceed;
    },
  })

  // Shrink the ceiling once the handler binary is loaded; the trap
  // pre-handler grows it back on the next wake via adjust_balloon.
  .on(LifecycleEvent.VmReady,    async (s) => { await s.vm.adjust_balloon(384); })
  .on(LifecycleEvent.VmShutdown, async (s) => { await s.ledger.flush(); });

async function main() {
  const session = await runtime.boot();
  try {
    // Each parked connection emits one request through the vsock Ch1
    // splice. `handleIngress` returns responses in arrival order and
    // writes them back over the same channel; the loop body is a no-op
    // so the example can count traffic for the log line.
    let served = 0;
    for await (const _ of session.handleIngress<WebhookRequest, WebhookResponse>({
      cwd: "/workspace/webhook",
      handler: handleWebhook,
    })) {
      served++;
    }
    console.log(`[kelir] served ${served} webhook(s) across wake cycles`);
  } finally {
    await session.suspend("/tmp/webhook-idle.snap");
    await session.terminate();
  }
}

main().catch(console.error);
