# ILLUSTRATIVE, NON-STANDALONE — do not import this file directly.
# It pins the Python scope protocol shapes for the unified builder; the
# builder class lives in cans/artifacts/code/python-plugin-builder.py
# and the normative contract is cans/02-plugin-model.md#Nested-scopes.
#
# Owned by: cans/02-plugin-model.md#Nested-scopes
# see: cans/02-plugin-model.md#Nested-scopes
# see: cans/02-plugin-model.md#Chain-decisions
# see: cans/artifacts/code/python-plugin-builder.py
# see: cans/artifacts/code/scope-builders.rs

from __future__ import annotations

from typing import Any, Callable, Protocol, Type, TypeVar, runtime_checkable

from kelir.sdk_errors import EConfig

T = TypeVar("T")


# see: cans/02-plugin-model.md#Nested-scopes
# Scope typing pin, Python leg: every scope is a Protocol. A scope without
# the member a closure calls fails the type check (mypy/pyright): a
# TrapScope protocol has no on_fs_write, so f.on_trap(...) inside .fs(...)
# is a type error. There is no .end() in Python: the closure returns the
# scope or None.
@runtime_checkable
class FsScopeProtocol(Protocol):
    # Ring verbs of the fs scope; scopes never cross.

    def on_fs_lookup(self, handler: Callable[..., Any]) -> "FsScopeProtocol":
        ...

    def on_fs_read(self, handler: Callable[..., Any]) -> "FsScopeProtocol":
        ...

    def on_fs_write(self, handler: Callable[..., Any]) -> "FsScopeProtocol":
        ...

    def on_fs_lock(self, handler: Callable[..., Any]) -> "FsScopeProtocol":
        ...

    # Shared scope sub-nodes.
    def config(
        self,
        lock_broker: Any = None,
        carryover_bytes: int | None = None,
        scan_engine: str | None = None,
        snoops: list[Any] | None = None,
    ) -> "FsScopeProtocol":
        ...

    def require(self, plugin: type | str) -> "FsScopeProtocol":
        ...

    def fork_with(self, fn: Callable[..., Any]) -> "FsScopeProtocol":
        ...

    def timeout(self, seconds: float) -> "FsScopeProtocol":
        ...

    def priority(self, i: int) -> "FsScopeProtocol":
        ...

    def rules(self, rules: Any) -> "FsScopeProtocol":
        ...


@runtime_checkable
class TrapScopeProtocol(Protocol):
    def on_trap(self, handler: Callable[..., Any]) -> "TrapScopeProtocol":
        ...

    def on_process_exit(self, handler: Callable[..., Any]) -> "TrapScopeProtocol":
        ...

    def config(self, cfg: Any) -> "TrapScopeProtocol":
        ...

    def require(self, plugin: type | str) -> "TrapScopeProtocol":
        ...

    def fork_with(self, fn: Callable[..., Any]) -> "TrapScopeProtocol":
        ...

    def timeout(self, seconds: float) -> "TrapScopeProtocol":
        ...

    def priority(self, i: int) -> "TrapScopeProtocol":
        ...

    def rules(self, rules: Any) -> "TrapScopeProtocol":
        ...


# see: cans/02-plugin-model.md#Nested-scopes
# The five orphan-verb scopes as Protocols: actions, streams, vsock,
# params, deny. TS spelling rides typescript-scope-interface.ts.
@runtime_checkable
class ActionsScopeProtocol(Protocol):
    def register(
        self, name: str, schema: dict[str, Any], handler: Callable[..., Any]
    ) -> "ActionsScopeProtocol":
        ...


@runtime_checkable
class StreamsScopeProtocol(Protocol):
    def register(
        self, kind: int, schema: dict[str, Any], handler: Callable[..., Any]
    ) -> "StreamsScopeProtocol":
        ...


@runtime_checkable
class VsockScopeProtocol(Protocol):
    def service(
        self, port: int, service: str, handler: Callable[..., Any]
    ) -> "VsockScopeProtocol":
        ...


@runtime_checkable
class ParamsScopeProtocol(Protocol):
    def model(self, model: type) -> "ParamsScopeProtocol":
        ...


@runtime_checkable
class DenyScopeProtocol(Protocol):
    def on_trap_deny(
        self, handler: Callable[..., Any]
    ) -> "DenyScopeProtocol":
        ...


# see: cans/02-plugin-model.md#Nested-scopes
# The negative vector: f.on_trap(...) inside .fs(...) is a type error in
# Python. Uncommenting the body must fail mypy/pyright with
# "Protocol FsScopeProtocol has no attribute on_trap". vectors.json
# cannot express an expected-error case (its schema carries
# event/expected_verdict only), so the negative case is pinned here and
# in the example file as a comment-pinned block.
#
# def negative_fs_on_trap(builder: KelirBuilder) -> KelirBuilder:
#     return builder.fs(
#         lambda f: f.on_trap(lambda event, ctx: Verdict.pass_())
#     )

# see: cans/02-plugin-model.md#Nested-scopes
# Runtime backstop: a scope object that fails the Protocol check raises
# EConfig at the call site, so the closure cannot silently no-op.
def _assert_scope(scope: Any, protocol: type) -> None:
    if not isinstance(scope, protocol):
        raise EConfig(f"scope {type(scope).__name__} does not implement {protocol.__name__}")
