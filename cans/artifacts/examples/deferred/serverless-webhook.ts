// see: cans/08-lifecycle.md#State-machine

import { kelir, t, PostExecAction, SuspendReason,
         type ExecutionReceipt, type LedgerHandle } from "@kelir/core";
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

async function handleWebhook(req: WebhookRequest, session: any): Promise<WebhookResponse> {
  const res = await session.spawn("/usr/bin/webhook-worker", ["--id", req.id], {
    input: req.body,
  });
  return {
    status: res.exitCode === 0 ? 200 : 500,
    headers: { "content-type": "application/json" },
    body: new TextEncoder().encode(JSON.stringify({ ack: req.id, code: res.exitCode })),
  };
}

async function main() {
  const session = await kelir({
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
    .use(mcp({ denyServerSampling: true }))

    .use(toolGuard({
      // see: cans/plugins/tool-guard.md#Immutable-schema-locking
      pinSchemas: true,
      detectCrossServerExfil: true,
      detectConfusableNames: true,
      scanToolResults: true,
      denySelfApproval: true,
      holdTtlSeconds: 300,
      // see: cans/plugins/tool-guard.md#Call-quota
      maxInvocationsPerTrace: 50,
    }))

    // see: cans/plugins/ledger.md#Merkle-WAL
    .use(ledger({
      storePath: ".kelir/webhook.merkle.db",
      strict: true,
      ocsfMapping: true,
      merkleChain: true,
      otel: { propagateW3cTraceparent: true, serviceName: "kelir-webhook" },
    }))

    .wire((w) => {
      // Transparent L4 divert to host Go proxies without TAP or netns
      w.divertPortToUnix(5433, "/run/agentsh/pgwire.sock");
      w.divertPortToUnix(8000, "/run/agentsh/mcp.sock");
    })

    .params((p) =>
      p.model(
        t.Object({
          action: t.Literal("dispatch-webhook"),
          webhookId: t.String(),
        }),
      ),
    )
    .trap("dispatch-webhook", {
      pre: async (c) => {
        await c.vm.adjustBalloon(512);
        return c.pass();
      },
      // see: cans/10-attestation.md#Execution-receipts
      post: async (c) => {
        const receipt: ExecutionReceipt = await c.receipt();
        if (receipt.exitKind.kind !== "normal") {
          return PostExecAction.RollbackAndHalt;
        }
        if (receipt.dirtyBytes === 0) return PostExecAction.RollbackAndHalt;
        return PostExecAction.Proceed;
      },
    })
    .lifecycle((l) =>
      l
        .onVmReady(async (ctx) => {
          await ctx.vm.adjustBalloon(384);
        })
        .onVmShutdown(async (ctx) => {
          await ctx.cap<LedgerHandle>("ledger")?.flush();
        }),
    )
    .seal()
    .boot();
  try {
    let served = 0;
    for await (const stream of session.handleIngress<WebhookRequest, WebhookResponse>(8443)) {
      const response = await handleWebhook(stream.request, session);
      await stream.respond(response);
      served++;
      if (served >= 1) break;
    }
    console.log(`[kelir] served ${served} webhook(s) across wake cycles`);
  } finally {
    await session.suspend(SuspendReason.external("human-authorization-hold"));
    await session.terminate();
  }
}

main().catch(console.error);
