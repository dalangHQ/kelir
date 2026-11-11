# Contributing to Kelir

Execution discipline for human contributors. Read this before touching a line of code.

---

## 1. Quickstart (Under 60 Seconds)

Kelir compiles with minimal footprint. Do not run blind workspace builds—target your specific crate to protect disk and cycle times.

```sh
# 1. Install prerequisites
rustup default 1.85
cargo install just cargo-nextest

# 2. Configure lean workspace (disables heavy debuginfo and bloat)
just dev-min

# 3. Verify your target crate immediately
just check-one kelir-core
```

---

## 2. Hard Invariants (Zero Exceptions)

CI rejects your pull request automatically if any invariant below is broken.

### The Comment Law
Only **one-line comments** of **three exact forms** are permitted in `crates/`, `packages/ts/`, and `packages/py/`:

| Format | Pattern | Permitted Use |
|---|---|---|
| `see:` | `// see: cans/05-storage-plane.md#Containment` | Points directly to the owning CANS spec anchor. |
| `TODO:` | `// TODO: wire to the live per-process receipt.` | Explicit L2 flow marker. |
| `SAFETY:` | `// SAFETY: pointer validated by virtqueue descriptor bounds.` | Mandatory justification directly above unsafe blocks. |

*Banned:* Multi-line comments, docstrings (`///`, `//!`, `"""`), prose explanations, banners, and trailing comments. 

### Code Hygiene
* **Zero Panic:** `unwrap()`, `expect()`, `panic!`, and `todo!()` are denied workspace-wide. Return `Result<T, E>` and bubble with `?`.
* **File Ceilings:** 1,000 LOC hard ceiling per file. 500 LOC target.
* **Test Segregation:** Zero `#[cfg(test)]` blocks in `src/`. All tests belong in sibling `tests/` directories.
* **Memory Safety:** `#![forbid(unsafe_code)]` workspace-wide. Unsafe is quarantined strictly to `kelir-drivers`, `kelir-transport`, and `kelir-shim`.
* **Current Lifecycle Phase:** **L1/L2 only.** Wire skeletons, types, error paths, and invocation dispatch. Do not author unprompted L3 deep business logic.

---

## 3. Source of Truth Protocol

Do not invent APIs, architectures, or protocols in code.

```
cans/                → Normative rules, threat models, and architectural invariants.
cans/artifacts/      → Concrete data layouts, wire schemas, trait definitions.
crates/ & packages/  → Mechanical implementations of the above.
```

1. **Find the spec:** Check `cans/` before changing behavior.
2. **One canonical home:** If updating a contract, edit `cans/` first.
3. **Artifact boundaries:** Nodes in `cans/*.md` reference content in `cans/artifacts/` via `see:`. Never duplicate raw schemas across both.

---

## 4. Pre-Flight Checklist

Run this local battery before committing. If these fail locally, CI fails instantly.

```sh
# 1. Format and lint checks
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# 2. Repository hygiene (enforces comment laws, LOC limits, banned tokens)
just check-hygiene

# 3. Spec and reference integrity
just check-spec

# 4. Fast unit test pass
just test-all
```

---

## 5. Pull Request Standards

* **Atomic Scope:** One architectural concern per pull request. 
* **Tiers & Dependencies:** Downward dependencies only. Lower-tier crates cannot import higher-tier plugins or engine facades.
* **Multi-Language Parity:** Core FFI changes must update vectors in `vectors.json` and cleanly reconcile against TypeScript (`packages/ts`) and Python (`packages/py`) bindings.
* **No Broken Commits:** Rebase against upstream `main`. Do not open merge-conflict PRs or force-push broken trees.
