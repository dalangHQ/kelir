# ILLUSTRATIVE EXCERPT — not standalone-executable.
#
# Demonstrates Python SDK's fluent builder + Pydantic v2 validation.
# KelirBuilder[TCapabilities, TParams] carries the accumulated surface in
# PEP 695 type parameters; handlers read c.cap.<plugin_key> (statically
# projected) and c.params (typed by the Pydantic model class). No
# c.service(...) lookup exists.
#
# Full runnable example: artifacts/examples/autonomous-swe-agent.py
# Canonical spec home:    cans/03-sdk-architecture.md#Python-SDK

from pydantic import BaseModel, Field
from kelir import kelir, Decision, HandlerContext


class FixWebhookParams(BaseModel, frozen=True):
    action: str = Field(pattern=r"^fix-webhook$")
    issue_id: str


# 1. Caller instantiates plugin reference directly
policy = RepoPolicyPlugin()

# 2. Fluid builder chain returns only Builder
builder = kelir.builder(vcpus=4, memory_mb=4096).use(policy)

# 3. Handler reads typed plugin state from c.cap, statically projected
async def commit_gate(c: HandlerContext) -> Decision:
    policy_svc = c.cap.policy
    if policy_svc.violations > 0:
        return Decision.deny(1, f"EPOLICY: {policy_svc.violations} violation(s)")
    return Decision.resume()


builder.trap(TrapFilter.binary("git").with_arg("commit"), commit_gate)


# 3. RAII Execution: __aenter__ boots microVM; __aexit__ guarantees termination
async with builder as session:
    # Native engine planes directly exposed on session
    await session.terminal.resize(120, 40)
    await session.spawn(
        "npm", ["test"],
        params=FixWebhookParams(action="fix-webhook", issue_id="ISSUE-8412"),
    )
