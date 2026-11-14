# cans/artifacts/code/session-accessors.py
#
# Unified session and hook-context accessors — content home.
# Three accessors, one meaning: session.plane(), session.cap(), c.cap().
# Python resolves by capability key (the plugin name); typing rides the
# .use() overloads.
#
# Owned by: cans/03-sdk-architecture.md#Service-lookup-across-languages

from __future__ import annotations

from typing import Optional, Protocol


# The plane set is closed; session.plane("<name>") is the one accessor.
PLANE_NAMES = (
    "vm", "terminal", "storage", "wire", "checkpoint", "host", "vsock",
)


class SessionAccessors(Protocol):
    # session.plane("vm") returns the vm plane handle; same for every
    # name in PLANE_NAMES.
    def plane(self, name: str) -> object: ...

    # session.cap("ledger") returns the LedgerCap instance, or None when
    # the ledger plugin is not mounted.
    def cap(self, name: str) -> Optional[object]: ...


# Documented sugar — every property resolves through session.plane():
#   session.vm         === session.plane("vm")
#   session.terminal   === session.plane("terminal")
#   session.storage    === session.plane("storage")
#   session.wire       === session.plane("wire")
#   session.checkpoint === session.plane("checkpoint")
#   session.host       === session.plane("host")
#   session.vsock      === session.plane("vsock")
#
# Deleted forms: session.ctx<T>("name"), session.ledger,
# session.cap.ledger attribute access.


class HandlerCtxAccessors(Protocol):
    # c.cap("repo-policy") returns the plugin's capability object inside
    # a hook, or None when not mounted.
    def cap(self, name: str) -> Optional[object]: ...


# Deleted forms: c["repo-policy"] indexing, c.cap.repo_policy attribute
# access, c.ctx, and c.<plugin_key> members.


# ── Plugin deps: single source of truth ───────────────────────────
#
# The class attribute populates the same registry that builder.boot()
# walks; there is no second, hand-maintained dependency list:
#
#     class RepoPolicyPlugin(BasePlugin):
#         name = "repo-policy"
#         requires: tuple[str, ...] = ("ledger",)
#
# See session-accessors.rs (#[derive(KelirDeps)]) and
# session-accessors.ts (requires("ledger")) for the same rule.
