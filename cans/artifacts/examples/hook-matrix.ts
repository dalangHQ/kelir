// ============================================================================
// hook-matrix.ts
// ============================================================================
// Canonical depth example: the custom hook surface, not the config
// surface. Where the swe-agent family configures wire/vfs/pty behavior
// (sinkhole dnsExcept, maskRead, tokenShield), this file implements the
// hooks themselves in one plugin, identically in hook-matrix.rs and
// hook-matrix.py.
//
// SAME HOOK SET in all three SDKs, in dispatch order:
//
//   1. onNetOutbound -> SyncVerdict. Frames containing the
//      documentation-range marker 198.51.100. yield to the host for
//      inspection (YieldToHost); all other frames Allow.
//   2. onFsLookup -> Some(FsInodeAttr) for exactly one path,
//      /workspace/.policy/identity, with mode 0o400 (no permission
//      fallback, per 02); None — fall through — everywhere else.
//   3. onFsRead -> sliding-window masking. A 31-byte tail is carried
//      between reads, so a secret token split across two reads is still
//      masked from the split point onward and no single read ever emits
//      a complete unmasked token (the token is 32 bytes).
//   4. onConsoleInput -> strips OSC52 clipboard sequences
//      (ESC ] 52 ; … terminator) from guest-bound input.
//   5. onConsoleOutput -> masks sk_live_ tokens in the chunk before it
//      reaches the host terminal.
//   6. onHeartbeat (SystemTelemetryParticipant in Rust) -> counts heartbeats; a receipt whose uptimeMs has
//      not advanced since the previous receipt increments the stall
//      counter (observation only, per 07).
//   7. onProcessExit -> records (pid, exitCode); non-zero exits
//      increment the failure counter.
//
// Masking is same-length (token bytes overwritten with "*"), because
// the read/output hooks mutate fixed-size buffers in place and cannot
// change a chunk's length.
//
// GUARDS (not new dispatch rows; hook order 1-7 above is unchanged):
//   - onFsLookup rejects a path whose `..` segments resolve outside
//     the /workspace root with E_PATH_TRAVERSAL (05), before the
//     identity check runs. Paths without `..` keep the old behavior:
//     the identity inode, else fall through.
//   - onFsWrite rejects any payload over the 64KB (65536-byte) FFI
//     inspection ceiling with BufferCeilingExceeded. That is the runtime
//     inspection ceiling, NOT the swe-agent policy diff ceiling, which
//     rejects with ENOSPC.
//
// GAPS (flagged, not silently invented):
//   - TS plugin-hook surface: no createPlugin hook method appears in any
//     TS example (only .state/.decorate). The .onNetOutbound /
//     .onFsLookup / .onFsRead / .onConsoleInput / .onConsoleOutput /
//     .onHeartbeat / .onProcessExit chain below mirrors the Rust trait
//     names in TS camelCase idiom. The one documented TS mechanism for
//     sync hot-path filters, .intercept(channel, filter, handler) (03),
//     names no channel strings, so it cannot express this hook set
//     without the same extrapolation.
//   - SyncVerdict spellings in TS: no SyncVerdict value appears in any
//     TS example. SyncVerdict.Allow / SyncVerdict.YieldToHost(0) below
//     follow the PostExecAction.RollbackAndHalt enum precedent in the
//     swe-agent TS example (imported from @kelir/core).
//   - YieldToHost's numeric argument: aa0e011 specifies it as a
//     delayed release in nanoseconds; 0 below is immediate.
//   - Rust ConsoleInterposer now takes a virtio-console port (u32)
//     before the byte buffer; the TS SDK surface here stays
//     bytes-only, as already flagged for createPlugin above.
//   - FsInodeAttr field names in TS: camelCase per TS idiom;
//     the Rust struct fields are the documented source (plugin-trait.rs).

import { kelir, createPlugin, SyncVerdict,
         type Session } from "@kelir/core";

// Guest path served as a synthetic inode, and the secret-token shape the
// masking hooks redact. TEST-NET-2 (198.51.100.0/24) is documentation
// space, used here as a denylist marker, not a real peer address.
const IDENTITY_PATH = "/workspace/.policy/identity";
const IDENTITY_CONTENT = "policy-matrix-identity\n";
const DENY_MARKER = "198.51.100.";
const TOKEN_PREFIX = "sk_live_";
const TOKEN_LEN = TOKEN_PREFIX.length + 24;
// Bytes carried between reads: one less than the token length, so any
// token split across a read boundary is reassembled in the window.
const CARRYOVER = TOKEN_LEN - 1;

// Lexical workspace escape check, `..` paths only: resolve `..`
// segments without touching the filesystem (no symlink resolution
// here; the reconciler's RESOLVE_BENEATH guard owns that) and report
// whether the resolved path left the /workspace root. Paths with no
// `..` segment are untouched by this guard.
function escapesWorkspace(path: string): boolean {
  const parts = path.split("/").filter((p) => p !== "" && p !== ".");
  if (!parts.includes("..")) return false;
  const stack: string[] = [];
  for (const part of parts) {
    if (part === "..") stack.pop();
    else stack.push(part);
  }
  return stack.length === 0 || stack[0] !== "workspace";
}

// ── Shared hook state (closed over, as in the swe-agent TS factory) ───────
const hookState = {
  tail: "" as string,
  heartbeats: 0,
  lastUptimeMs: null as number | null,
  stalls: 0,
  exits: 0,
  failures: 0,
};

// Overwrite every byte of a complete sk_live_ token (prefix + 24 alnum)
// with "*", in place. A token is only masked when all 32 bytes are
// visible in the window; partial prefixes at the window's end are left
// for the next read's tail, never emitted as a complete token.
function maskTokens(window: string): string {
  const chars = window.split("");
  for (let i = 0; i + TOKEN_LEN <= chars.length; i++) {
    const slice = chars.slice(i, i + TOKEN_LEN).join("");
    if (slice.startsWith(TOKEN_PREFIX) &&
        /^[A-Za-z0-9]{24}$/.test(slice.slice(TOKEN_PREFIX.length))) {
      for (let j = i; j < i + TOKEN_LEN; j++) chars[j] = "*";
      i += TOKEN_LEN - 1;
    }
  }
  return chars.join("");
}

const hookMatrixPlugin = createPlugin("hook-matrix")
  // Hook 1: wire outbound. Any frame carrying the denylist marker yields
  // to host inspection instead of leaving the TAP.
  .onNetOutbound(async (frame) => {
    const text = Buffer.from(frame).toString("latin1");
    if (text.includes(DENY_MARKER)) {
      // YieldToHost(u64) is a delayed release in nanoseconds; 0 is immediate.
      return SyncVerdict.YieldToHost(0);
    }
    return SyncVerdict.Allow;
  })
  // Hook 2: storage lookup. Exactly one synthetic inode, mode 0o400
  // with no permission fallback (02); everything else falls through.
  .onFsLookup(async (path) => {
    // GUARD: traversal first — an escaping path is rejected, never
    // served and never silently fallen through to the host fs.
    if (escapesWorkspace(path)) {
      throw new Error("E_PATH_TRAVERSAL: lookup escapes the /workspace root");
    }
    if (path !== IDENTITY_PATH) return null;
    return {
      mode: 0o400, uid: 0, gid: 0,
      size: IDENTITY_CONTENT.length,
      // GAP: hash semantics unspecified; 0 is an explicit placeholder.
      content: IDENTITY_CONTENT,
    };
  })
  // Hook 3: storage read, sliding-window masking. Window = carried tail
  // + this chunk; mask over the window, emit the chunk's portion, carry
  // the last CARRYOVER bytes forward. Invariant: a token split across
  // reads is masked from the split point onward in the later chunk, and
  // no single chunk ever contains a complete unmasked token, because
  // the earlier chunk held at most a 31-byte fragment of it.
  .onFsRead(async (_path, buffer) => {
    const window = hookState.tail + Buffer.from(buffer).toString("latin1");
    const masked = maskTokens(window);
    const chunk = masked.slice(masked.length - buffer.length);
    hookState.tail = masked.slice(-CARRYOVER);
    return Buffer.from(chunk, "latin1");
  })
  // GUARD (not a new dispatch row): the 64KB FFI inspection ceiling.
  // A payload over it is rejected with BufferCeilingExceeded — the
  // runtime inspection error, distinct from the swe-agent policy diff
  // ceiling, which rejects oversize diffs with ENOSPC. GAP: no
  // BufferCeilingExceeded throw is attested in any TS example; the
  // swe-agent TS snoop throws Error("ENOSPC: …"), so the ceiling name
  // is carried as the Error message prefix here (see header).
  .onFsWrite(async (_path, buffer) => {
    const ffiInspectionCeiling = 64 * 1024;
    if (buffer.byteLength > ffiInspectionCeiling) {
      throw new Error(`BufferCeilingExceeded: payload ${buffer.byteLength} ` +
        `bytes exceeds ${ffiInspectionCeiling}-byte FFI ceiling`);
    }
    return SyncVerdict.Allow;
  })
  // Hook 4: PTY input. Strip OSC52 clipboard exfiltration sequences:
  // ESC ] 52 ; … terminated by BEL (0x07) or ST (ESC \).
  .onConsoleInput(async (bytes) => {
    const text = Buffer.from(bytes).toString("latin1");
    const stripped = text.replace(/\x1b\]52;[^\x07\x1b]*(?:\x07|\x1b\\)?/g, "");
    return Buffer.from(stripped, "latin1");
  })
  // Hook 5: PTY output. Single-chunk masking; the cross-chunk case is
  // the vfs read path's sliding window above, not the terminal path.
  .onConsoleOutput(async (chunk) => {
    const masked = maskTokens(Buffer.from(chunk).toString("latin1"));
    return Buffer.from(masked, "latin1");
  })
  // Hook 6: heartbeat. Observation only (07): count, and note stalls,
  // but never gate.
  .onHeartbeat(async (event) => {
    if (hookState.lastUptimeMs === event.uptimeMs && hookState.heartbeats > 0) {
      hookState.stalls += 1;
    }
    hookState.lastUptimeMs = event.uptimeMs;
    hookState.heartbeats += 1;
  })
  // Hook 7: process exit. Record, and count non-zero exits as failures.
  .onProcessExit(async (event) => {
    hookState.exits += 1;
    if (event.exitCode !== 0) hookState.failures += 1;
  });

// ============================================================================
// BOOT — exercise the hooks, report counters
// ============================================================================

const runtime = kelir({
  vcpus: 2, memoryMb: 2048, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-hooks.erofs",
  tscLocked: true, enableWatchdog: true,
}).use(hookMatrixPlugin);

async function main() {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  // Reads the synthetic inode (lookup hook) through the masking read
  // path, and produces process-exit + heartbeat traffic for the trap
  // hooks. PTY input/output hooks fire on the session's terminal.
  const probe = await session.spawn("cat", [IDENTITY_PATH],
                                    { cwd: "/workspace" });
  console.log(`[kelir] cat identity -> exit=${probe.exitCode}`);

  // Let at least one heartbeat receipt arrive before reporting.
  await new Promise((r) => setTimeout(r, 100));
  console.log(`[kelir] heartbeats=${hookState.heartbeats} ` +
              `stalls=${hookState.stalls} exits=${hookState.exits} ` +
              `failures=${hookState.failures}`);

  await session.terminate();
}

main().catch(console.error);
