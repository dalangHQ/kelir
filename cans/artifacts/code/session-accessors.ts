// cans/artifacts/code/session-accessors.ts
//
// Unified session and hook-context accessors — content home.
// Three accessors, one meaning: session.plane(), session.cap(), c.cap().
// TypeScript resolves by capability key (the plugin name); the return
// type comes from the accumulated Ctx of .use(plugin).
//
// Owned by: cans/03-sdk-architecture.md#Service-lookup-across-languages

// The plane set is closed; session.plane("<name>") is the one accessor.
export type PlaneName =
  | "vm"
  | "terminal"
  | "storage"
  | "wire"
  | "checkpoint"
  | "host"
  | "vsock";

// Documented sugar — every property resolves through session.plane():
//   session.vm        === session.plane("vm")
//   session.terminal  === session.plane("terminal")
//   session.storage   === session.plane("storage")
//   session.wire      === session.plane("wire")
//   session.checkpoint=== session.plane("checkpoint")
//   session.host      === session.plane("host")
//   session.vsock     === session.plane("vsock")

// session.cap("ledger") returns the plugin's capability object typed by
// the accumulated Ctx, or undefined when the plugin is not mounted.
// session.ctx<T>("name"), session.ledger, and session.cap.ledger
// (attribute access) are deleted forms.
export interface SessionCaps<TCaps extends Record<string, unknown>> {
  plane(name: PlaneName): Plane;
  cap<K extends keyof TCaps & string>(name: K): TCaps[K] | undefined;
}

// c.cap("repo-policy") — the same resolution inside a hook, typed by
// the accumulated Ctx, undefined when not mounted.
// c["repo-policy"] indexing, c.cap.repo_policy attribute access, and
// c.<pluginKey> members are deleted forms.
export interface HandlerCtxCaps<TCtx extends Record<string, unknown>> {
  cap<K extends keyof TCtx & string>(name: K): TCtx[K] | undefined;
}

// ── Plugin deps: single source of truth ───────────────────────────
//
// requires("ledger") populates Deps — the same registry that seal()
// validates and boot() walks. There is no second, hand-maintained
// dependency list:
//
//   createPlugin("repo-policy").requires("ledger")
//     // Deps = "ledger"; .use() rejects a Deps not in the caller Ctx
//
// See session-accessors.rs (#[derive(KelirDeps)]) and
// session-accessors.py (requires: tuple[str, ...]) for the same rule.
