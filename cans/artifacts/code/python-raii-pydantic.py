# ILLUSTRATIVE EXCERPT — not standalone-executable.
#
# Demonstrates Python SDK's fluent builder + Pydantic v2 validation.
# Host holds instantiated plugin reference; handlers read c.policy.
# SyntheticFile: FUSE-presented paths with no host backing file.
# Verification Reject: Decision.deny returns a POSIX errno.
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

# 3. Handler reads typed plugin state directly from context c
async def commit_gate(c: HandlerContext) -> Decision:
    if c.policy.violations > 0:
        return Decision.deny(1, f"EPOLICY: {policy.violations} violation(s)")
    return Decision.resume()


builder.exec(lambda cmd: cmd.before("git commit *", commit_gate))


# 3. RAII Execution: __aenter__ boots microVM; __aexit__ guarantees termination
async with builder as session:
    await session.spawn(
        "npm", ["test"],
        params=FixWebhookParams(action="fix-webhook", issue_id="ISSUE-8412"),
    )
