# ILLUSTRATIVE EXCERPT — not standalone-executable.
#
# Demonstrates Python SDK's assembly-time closure validation + Pydantic v2.
# Builder inspects closure freevars during .exec(); unbound references
# raise KelirConfigurationError before hypervisor boots.
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


# 1. Composite plugin tree unpacked at assembly time
class SecurityBundle:
    def __init__(self, policy: RepoPolicyPlugin):
        self.policy = policy

    def unpack(self, builder, scope=None):
        builder, policy_h = builder.plugin_with_handle(self.policy, scope=scope)
        return builder, policy_h

# 2. Builder consumes composite bundle and captures typed handle
builder = kelir.builder(vcpus=4, memory_mb=4096)
builder, policy = builder.use_composite(SecurityBundle(RepoPolicyPlugin()))


# 2. Handler captures handle via closure
async def commit_gate(_c: HandlerContext) -> Decision:
    if policy.violations > 0:
        return Decision.deny(1, f"EPOLICY: {policy.violations} violation(s)")
    return Decision.resume()


builder.exec(lambda cmd: cmd.before("git commit *", commit_gate))


# 3. RAII Execution: __aenter__ boots microVM; __aexit__ guarantees termination
async with builder as session:
    await session.spawn(
        "npm", ["test"],
        params=FixWebhookParams(action="fix-webhook", issue_id="ISSUE-8412"),
    )
