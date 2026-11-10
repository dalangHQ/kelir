# Contributing — disk-conscious setup

A full workspace check costs ~30G of `target/` (19G debug, 7.8G incremental).
Contributors on small disks build as little as possible:

- One crate, not the workspace: `just check-one <crate>` (for example
  `just check-one kelir-core`). It checks under the `dev-min` profile:
  incremental off, debug symbols off.
- Bare `cargo check` builds default members only. Heavy drivers
  (`driver-rustvmm`, `driver-hvf`, `driver-openvmm`, `driver-avf`),
  `kelir-integration`, and the `napi`/`pyo3` bindings stay opt-in via `-p`.
- Prune stale artifacts with `just clean-old` (cargo-sweep, older than
  7 days). Full `cargo clean` is the last resort, never routine.
- Prefer `cargo check` over `cargo build` when only compilation checks
  are needed; keep `sccache` enabled (`just bootstrap` installs it
  alongside `cargo-nextest` and `cargo-sweep`).
- Realistic minimum after the above: ~2-3G (217M source, registry,
  toolchain, one crate's check artifacts), not ~30G.

Spec first: behaviour and CI rules live in `cans/15-operations.md`;
`cans/artifacts/config/` holds the canonical `justfile` and `ci.yml`.
Keep the root copies and the mirrors byte-identical. Run `just pre-flight`
before pushing; see `AGENTS.md` for the full gate list.
