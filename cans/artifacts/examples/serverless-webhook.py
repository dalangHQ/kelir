# see: cans/08-lifecycle.md#State-machine
# see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
# see: cans/plugins/mcp.md#Plugin-mcp
# see: cans/plugins/tool-guard.md#Plugin-tool-guard
# see: cans/plugins/ledger.md#Plugin-ledger
# see: cans/plugins/step-eval.md#Plugin-step-eval
from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Literal

import kelir
from kelir import (
    BaseModel,
    ExecutionReceipt,
    External,
    ExternalSuspendKind,
    Field,
    HandlerContext,
    KelirBuilder,
    LifecycleEvent,
    PostExecAction,
    Verdict,
)
from kelir.plugins.plugin_wire_ingress import (
    BacklogConfig,
    ListenConfig,
    ParkingConfig,
    WakeOnPacketConfig,
    WireIngressConfig,
    WireIngressPlugin,
)
from kelir.plugins.plugin_mcp import McpConfig, McpPlugin
from kelir.plugins.plugin_tool_guard import (
    ToolGuardConfig, ToolGuardPlugin,
)
from kelir.plugins.plugin_ledger import (
    LedgerConfig, LedgerPlugin, OtelConfig, ReceiptConfig,
)


@dataclass
class WebhookRequest:
    id: str
    headers: dict[str, str] = field(default_factory=dict)
    body: bytes = b""


@dataclass
class WebhookResponse:
    status: int = 200
    headers: dict[str, str] = field(default_factory=dict)
    body: bytes = b""


class DispatchWebhook(BaseModel, frozen=True):
    action: Literal["dispatch-webhook"]
    webhook_id: str = Field(alias="webhookId")

    model_config = {"populate_by_name": True, "extra": "forbid"}


async def handle_webhook(req: WebhookRequest, session: kelir.Session) -> WebhookResponse:
    res = await session.spawn(
        "/usr/bin/webhook-worker",
        ["--request-id", req.id],
        input=req.body,
    )
    return WebhookResponse(
        status=200 if res.exit_code == 0 else 500,
        body=res.stdout,
    )


def build_runtime() -> KelirBuilder:
    async def pre_dispatch(c: HandlerContext) -> Verdict:
        await c.vm.adjust_balloon(512)
        return Verdict.pass_()

    # see: cans/10-attestation.md#Execution-receipts
    async def post_dispatch(c: HandlerContext) -> PostExecAction:
        receipt: ExecutionReceipt = await c.receipt()
        if receipt.dirty_bytes == 0:
            return PostExecAction.rollback_and_halt()
        return PostExecAction.proceed()

    return (
        kelir.builder(
            vcpus=2, memory_mb=512, driver="auto",
            kernel="./fixtures/vmlinux-6.6-arm64-kelir",
            rootfs_erofs="./fixtures/rootfs-webhook.erofs",
            tsc_locked=True, enable_watchdog=True,
        )

        # see: cans/plugins/wire-ingress.md#Connection-parking-on-suspend
        # see: cans/plugins/wire-ingress.md#On-rollback:-parked-connections-get-TCP-RST-(post-handshake:-503-close)
        .use(WireIngressPlugin(WireIngressConfig(
            listen=ListenConfig(port=8443, protocol="https"),
            parking=ParkingConfig(
                max_buffered_bytes=64 * 1024, idle_timeout_ms=30_000,
            ),
            # see: cans/plugins/wire-ingress.md#Ingress-stream-framing-on-stream-kind-L4-FORWARD-(mux-0x0001):
            wake_on_packet=WakeOnPacketConfig(transport_channel=1),
            # see: cans/plugins/wire-ingress.md#Overflow
            backlog=BacklogConfig(max_connections=256, overflow="reject-503"),
        )))

        # see: cans/plugins/mcp.md#Stream-kind-MCP-CONSOLE-routes-directly-to-MCP-client-without-PTY-conflict
        .use(McpPlugin(McpConfig()))

        .use(ToolGuardPlugin(ToolGuardConfig(
            # see: cans/plugins/tool-guard.md#Immutable-schema-locking
            pin_schemas=True,
            # see: cans/plugins/tool-guard.md#Call-quota
            max_invocations_per_trace=50,
        )))

        # see: cans/plugins/ledger.md#Merkle-WAL
        .use(LedgerPlugin(LedgerConfig(
            store="sqlite://.kelir/webhook.merkle.db",
            merkle_chain=True,
            otel=OtelConfig(
                service_name="kelir-webhook",
                propagate_w3c_traceparent=True,
            ),
            receipts=ReceiptConfig(),
        )))
        .params(DispatchWebhook)
        .trap("dispatch-webhook", pre=pre_dispatch, post=post_dispatch)
        .on(LifecycleEvent.VmReady, lambda ctx: ctx.vm.adjust_balloon(384))
        .on(LifecycleEvent.VmShutdown, lambda ctx: ctx.cap("ledger").flush())
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        served = 0
        async for stream in session.handle_ingress(
            port=8443,
            request_type=WebhookRequest,
            response_type=WebhookResponse,
        ):
            response = await handle_webhook(stream.request, session)
            await stream.respond(response)
            served += 1
            if served >= 1:
                break

        print(f"[kelir] served {served} webhook(s) across wake cycles")
        await session.suspend(
            External(ExternalSuspendKind.HUMAN_AUTHORIZATION_HOLD),
        )


if __name__ == "__main__":
    asyncio.run(main())
