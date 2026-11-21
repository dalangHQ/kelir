# see: cans/08-lifecycle.md#State-machine
# see: cans/plugins/wire-ingress.md#Plugin-wire-ingress
# see: cans/plugins/mcp.md#Plugin-mcp
# see: cans/plugins/tool-guard.md#Plugin-tool-guard
# see: cans/plugins/ledger.md#Plugin-ledger
# see: cans/plugins/step-eval.md#Plugin-step-eval
from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any, AsyncIterator, Literal, cast

import kelir
from kelir import (
    BaseModel,
    ExecutionReceipt,
    ExitKindNormal,
    External,
    Field,
    HandlerContext,
    KelirBuilder,
    PostExecAction,
    Verdict,
)
from kelir.plugin_model_flow import LifecycleCtx
from kelir.plugins.wire_ingress import (
    BacklogConfig,
    ListenConfig,
    ParkingConfig,
    WakeOnPacketConfig,
    WireIngressConfig,
    WireIngressPlugin,
)
from kelir.plugins.mcp import McpConfig, McpPlugin
from kelir.plugins.tool_guard import (
    ToolGuardConfig, ToolGuardPlugin,
)
from kelir.plugins.ledger import (
    LedgerConfig, LedgerPlugin, OtelConfig, ReceiptConfig,
)
from kelir.wire_planes import WireConfig


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
        if receipt.exit_kind != ExitKindNormal():
            return PostExecAction.rollback_and_halt()
        assert receipt.wall_clock_ns > 0
        if receipt.dirty_bytes == 0:
            return PostExecAction.rollback_and_halt()
        return PostExecAction.proceed()

    def _build_wire() -> WireConfig:
        wire = WireConfig()
        wire.userspace_netstack = True
        wire.default_egress = "drop"
        wire.divert_port_to_unix(5433, "/run/agentsh/pgwire.sock")
        wire.divert_port_to_unix(8000, "/run/agentsh/mcp.sock")
        return wire

    async def _vm_ready(ctx: LifecycleCtx) -> None:
        await ctx.vm.adjust_balloon(384)

    async def _vm_shutdown(ctx: LifecycleCtx) -> None:
        ledger = ctx.cap("ledger")
        if ledger is not None:
            await ledger.flush()

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
        .use(McpPlugin(McpConfig(deny_server_sampling=True)))

        .use(ToolGuardPlugin(ToolGuardConfig(
            # see: cans/plugins/tool-guard.md#Immutable-schema-locking
            pin_schemas=True,
            detect_cross_server_exfil=True,
            detect_confusable_names=True,
            scan_tool_results=True,
            deny_self_approval=True,
            hold_ttl_seconds=300,
            # see: cans/plugins/tool-guard.md#Call-quota
            max_invocations_per_trace=50,
        )))

        # see: cans/plugins/ledger.md#Merkle-WAL
        .use(LedgerPlugin(LedgerConfig(
            store_path=".kelir/webhook.merkle.db",
            strict=True,
            ocsf_mapping=True,
            merkle_chain=True,
            otel=OtelConfig(
                service_name="kelir-webhook",
                propagate_w3c_traceparent=True,
            ),
            receipts=ReceiptConfig(),
        )))
        .wire(_build_wire())
        .params(lambda p: p.model(DispatchWebhook))
        .trap("dispatch-webhook", pre=pre_dispatch, post=post_dispatch)
        .lifecycle(
            lambda l: l.on_vm_ready(_vm_ready)
            .on_vm_shutdown(_vm_shutdown)
        )
        .seal()
    )


async def main() -> None:
    async with build_runtime() as session:
        served = 0
        # handle_ingress is declared `-> Any` but yields ingress streams
        # (async-generator function); the cast pins the async-iterator
        # shape pyright cannot infer from the declared return type.
        streams = cast(
            AsyncIterator[Any],
            session.handle_ingress(
                port=8443,
                request_type=WebhookRequest,
                response_type=WebhookResponse,
            ),
        )
        async for stream in streams:
            response = await handle_webhook(stream.request, session)
            await stream.respond(response)
            served += 1
            if served >= 1:
                break

        print(f"[kelir] served {served} webhook(s) across wake cycles")
        await session.suspend(
            External("human-authorization-hold"),
        )


if __name__ == "__main__":
    asyncio.run(main())
