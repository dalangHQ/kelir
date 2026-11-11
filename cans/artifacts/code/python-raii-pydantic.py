# ILLUSTRATIVE EXCERPT — not standalone-executable.
#
# Demonstrates Python SDK's flat builder + Pydantic v2 validation.
# KelirBuilder stores plugins alongside Pydantic models; .use(plugin)
# appends to the registry and returns self, overloaded per plugin so
# c.cap.<plugin_key> is typed and c.params is typed by the Pydantic
# model class. Prerequisite validation runs at builder.boot() and
# raises EConfig on unfulfilled requirements. No c.service(...) lookup
# exists, and no generic type-parameter accumulation is used.
#
# Full runnable example: artifacts/examples/autonomous-swe-agent.py
# Canonical spec home:    cans/03-sdk-architecture.md#Python-SDK

from pydantic import BaseModel, Field
from kelir import kelir, Verdict, HandlerContext


class FixWebhookParams(BaseModel, frozen=True):
    action: str = Field(pattern=r"^fix-webhook$")
    issue_id: str


# 1. Caller instantiates plugin reference directly
policy = RepoPolicyPlugin()

# 2. Flat builder chain: .use() appends and returns self
builder = kelir.builder(vcpus=4, memory_mb=4096).use(policy)

# 3. Handler reads typed plugin state from c.cap, statically projected.
# c.cap.policy is populated by .use(policy) at builder.boot(), not before:
# the capability struct is materialized from the plugin's Context before
# the first handler invocation; it is not a stub.
# Verdict methods: pass_() / patch() / drop(errno, reason) /
# suspend(until) / abort(reason). Drop errno None = silent blackhole,
# chaos loss only; a policy denial carries an errno (EPERM used here
# as the generic POSIX denial — no policy-specific errno is pinned
# in the grounded spec facts).
async def commit_gate(c: HandlerContext) -> Verdict:
    policy_svc = c.cap.policy
    if policy_svc.violations > 0:
        return Verdict.drop("EPERM", f"EPOLICY: {policy_svc.violations} violation(s)")
    return Verdict.pass_()


builder.trap(TrapFilter.binary("git").with_arg("commit"), commit_gate)


# 3. RAII Execution: __aenter__ boots microVM; __aexit__ calls
# session.terminate() on any exception
async with builder as session:
    # Native engine planes directly exposed on session
    await session.terminal.resize(120, 40)
    await session.spawn(
        "npm", ["test"],
        params=FixWebhookParams(action="fix-webhook", issue_id="ISSUE-8412"),
    )
