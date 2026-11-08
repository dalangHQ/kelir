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


  .use(wireIngress({


    listen: { port: 8443, protocol: "https" },
    parking: { maxBufferedBytes: 64 * 1024, idleTimeoutMs: 30_000 },


    wakeOnPacket: { transportChannel: 1 },


    backlog: { maxConnections: 256, overflow: "reject-503" },
  }))


  .use(mcp({


    transport: { streamKind: "MCP_CONSOLE", framing: "json-rpc-2.0" },
    advertise: { tools: ["webhook.dispatch", "webhook.replay"] },
  }))


  .use(toolGuard({


    pinSchemas: true,

    maxInvocationsPerTrace: 50,
    approvals: {
      "webhook.replay": { action: "hold", durationMs: 500 },
    },
  }))

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


    pre: async (c) => {
      await c.vm.adjustBalloon(512);
      return c.pass();
    },
    post: async (c) => {
      const receipt: ExecutionReceipt = await c.receipt();


      if (receipt.dirtyBytes === 0) return PostExecAction.RollbackAndHalt;
      return PostExecAction.Proceed;
    },
  });

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
