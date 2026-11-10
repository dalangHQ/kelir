// see: cans/08-lifecycle.md#State-machine

import { kelir, t, PostExecAction,
         type ExecutionReceipt } from "@kelir/core";
import { wireIngress, mcp, toolGuard, ledger } from "@kelir/core";

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

  // see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
  // see: cans/plugins/wire-ingress.md#On-rollback:-parked-connections-get-TCP-RST-(post-handshake:-503-close)
  .use(wireIngress({
    listen: { port: 8443, protocol: "https" },
    parking: { maxBufferedBytes: 64 * 1024, idleTimeoutMs: 30_000 },
    // see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
    wakeOnPacket: { transportChannel: 1 },
    // see: cans/plugins/wire-ingress.md#Overflow
    backlog: { maxConnections: 256, overflow: "reject-503" },
  }))

  // see: cans/plugins/mcp.md#Stream-kind-MCP-CONSOLE-routes-directly-to-MCP-client-without-PTY-conflict
  // TODO: live McpConfig has no transport/advertise fields; SSOT transport MCP_CONSOLE + advertise webhook.dispatch/webhook.replay dropped as config, see cans/plugins/mcp.md#Transport:-JSON-RPC-2.0-over-MCP-CONSOLE-mux-stream
  .use(mcp({}))

  // TODO: live ToolGuardConfig has no approvals field; SSOT approvals webhook.replay hold 500ms dropped as config, see cans/plugins/tool-guard.md#Host-approval-binds-strictly-to-canonical-arguments-and-payload-hash
  .use(toolGuard({
    // see: cans/plugins/tool-guard.md#Immutable-schema-locking
    pinSchemas: true,
    // see: cans/plugins/tool-guard.md#Call-quota
    maxInvocationsPerTrace: 50,
  }))

  // see: cans/plugins/ledger.md#Merkle-WAL
  // TODO: SSOT otel.traceParent matches no live field; spec name is propagate_w3c_traceparent, live TS field propagateW3cTraceparent used, see cans/plugins/ledger.md#OpenTelemetry
  .use(ledger({
    store: "sqlite://.kelir/webhook.merkle.db",
    merkleChain: true,
    otel: { propagateW3cTraceparent: true, serviceName: "kelir-webhook" },
  }))

  .params(t.Object({
    action: t.Literal("dispatch-webhook"),
    webhookId: t.String(),
  }))
  // TODO: spec spells session.vm.adjust_balloon; live TS VmHandle is adjustBalloon, see cans/03-sdk-architecture.md#Session-control-surface
  .trap("dispatch-webhook", {
    pre: async (c) => {
      await c.vm.adjustBalloon(512);
      return c.pass();
    },
    // see: cans/10-attestation.md#Execution-receipts
    post: async (c) => {
      const receipt: ExecutionReceipt = await c.receipt();
      if (receipt.dirtyBytes === 0) return PostExecAction.RollbackAndHalt;
      return PostExecAction.Proceed;
    },
  });

// TODO: SSOT .on(VmReady) balloon shrink to 384 and .on(VmShutdown) ledger.flush dropped; live .on delivers LifecycleContext only, see cans/08-lifecycle.md#Session-and-context-handles
async function main() {
  const session = await runtime.boot();
  try {
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
