**Execution Philosophy:** Zero CPU burn. Sub-minute local cycles via `just`. Maximize GitHub Actions free tier (public repo = unlimited minutes, but macOS runners eat concurrency; private repo = 2,000 min ceiling).

---

## 1. Local Velocity: `justfile`

Requires: `just`, `cargo-nextest`, `mold` (Linux), `zig` or `musl-tools` (for the static shim), `pnpm`, `uv`.

```just
# kelir development orchestrator
set shell := ["bash", "-uc"]

default:
    @just --list

# --- 1. SETUP & TOOLCHAINS ---
bootstrap:
    cargo install cargo-nextest sccache --locked
    rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
    pnpm install --frozen-lockfile
    uv venv && uv pip install -e "packages/py[dev]"

# --- 2. LIGHTNING BUILD (<15s) ---
# Compiles host core using mold/lld linker and sccache
build-host:
    RUSTC_WRAPPER=sccache cargo build --workspace --exclude kelir-shim

# Compiles static 50KB in-guest musl init binary
build-shim target="aarch64-unknown-linux-musl":
    cargo build -p kelir-shim --release --target {{target}}
    @ls -lh target/{{target}}/release/kelir-shim

# Builds TS napi bindings
build-ts:
    pnpm --filter @kelir/core build

# --- 3. RAPID TEST ENGINE (<10s) ---
# Fast unit tests ignoring hardware-dependent hypervisor paths
test-unit:
    cargo nextest run --workspace --exclude kelir-shim --exclude driver-libkrun -E 'not test(test_hw_)'

# Runs anti-TOCTOU, PTRACE_SEIZE, and subreaper tests in isolated container/namespaces
test-shim:
    cargo nextest run -p kelir-shim

# Full TS context accumulation & type checking
typecheck:
    pnpm -r exec tsc --noEmit
    uv run mypy packages/py

# --- 4. PRE-FLIGHT CHECK (Run before git push) ---
pre-flight: build-shim
    cargo fmt --all -- --check
    cargo clippy --workspace --exclude kelir-shim -- -D warnings
    pnpm -r exec eslint .
    just test-unit
    just typecheck
```

---

## 2. GitHub Actions Free-Tier Matrix (`.github/workflows/ci.yml`)

*Strategy:* Public repos get free minutes, but macOS runners are scarce and drain private allotments at a **10x multiplier**. Run 90% of checks on Linux x86; run macOS Apple Silicon jobs strictly for the `driver-libkrun` compilation gate.

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

concurrency:
  group: ${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always
  SCCACHE_GHA_ENABLED: "true"
  RUST_BACKTRACE: 1

jobs:
  # ─── 1. FAST STATIC GATE (<90s) ────────────────────────────
  lint-and-types:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - uses: pnpm/action-setup@v3
        with:
          version: 9
      - uses: astral-sh/setup-uv@v2
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: mozilla-actions/sccache-action@v0.0.5

      - name: Rust Lints
        run: |
          cargo fmt --all --check
          cargo clippy --workspace --exclude kelir-shim -- -D warnings

      - name: Type Checks (TS & Python)
        run: |
          pnpm install --frozen-lockfile
          pnpm -r exec tsc --noEmit
          uv venv && uv pip install -e "packages/py[dev]"
          uv run mypy packages/py

  # ─── 2. GUEST SHIM CROSS-COMPILE ────────────────────────────
  build-guest-shim:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-musl, aarch64-unknown-linux-musl]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: mozilla-actions/sccache-action@v0.0.5
      - name: Install Musl Tools
        run: sudo apt-get update && sudo apt-get install -y musl-tools gcc-aarch64-linux-gnu
      - name: Compile Shim
        run: cargo build -p kelir-shim --release --target ${{ matrix.target }}
      - name: Verify Size (<100KB binary constraint)
        run: |
          SIZE=$(stat -c%s target/${{ matrix.target }}/release/kelir-shim)
          echo "Shim size: $SIZE bytes"
          test $SIZE -lt 102400 || (echo "SHIM BLOAT DETECTED" && exit 1)
      - uses: actions/upload-artifact@v4
        with:
          name: kelir-shim-${{{ matrix.target }}
          path: target/${{ matrix.target }}/release/kelir-shim

  # ─── 3. HOST LINUX TESTS (KVM Mocked) ──────────────────────
  test-linux:
    needs: [lint-and-types, build-guest-shim]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: taiki-e/install-action@nextest
      - uses: mozilla-actions/sccache-action@v0.0.5
      - name: Run Nextest Unit Suite
        run: cargo nextest run --workspace --exclude kelir-shim --exclude driver-libkrun

  # ─── 4. MACOS APPLE SILICON GATE (Zero Bloat) ──────────────
  check-macos-hvf:
    needs: [lint-and-types]
    runs-on: macos-14 # Apple Silicon M1 (Free tier on GitHub Actions for public repos)
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: mozilla-actions/sccache-action@v0.0.5
      # Only verify compilation of Apple Silicon HVF driver; do not waste runner minutes on full matrix
      - name: Check HVF libkrun driver
        run: cargo check -p driver-libkrun
```
