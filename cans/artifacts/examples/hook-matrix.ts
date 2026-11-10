// see: cans/02-plugin-model.md#Segregated-ring-traits

import { kelir, createPlugin, Verdict, FilterAction, SuspendUntil,
         type Session } from "@kelir/core";

// see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
const IDENTITY_PATH = "/workspace/.policy/identity";
const IDENTITY_CONTENT = "policy-matrix-identity\n";
const DENY_MARKER = "198.51.100.";
const TOKEN_PREFIX = "sk_live_";
const TOKEN_LEN = TOKEN_PREFIX.length + 24;
const CARRYOVER = TOKEN_LEN - 1;

// see: cans/05-storage-plane.md#Containment
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

const hookState = {
  tail: "" as string,
  heartbeats: 0,
  lastUptimeMs: null as number | null,
  stalls: 0,
  exits: 0,
  failures: 0,
};

// see: cans/05-storage-plane.md#In-flight-read-masking
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
  // see: cans/07-wire-plane.md#Frame-drop-and-forward-semantics
  .onNetOutbound((frame) => {
    const text = Buffer.from(frame).toString("latin1");
    if (text.includes(DENY_MARKER)) {
      return FilterAction.Suspend(SuspendUntil.deadline(0));
    }
    return FilterAction.Forward;
  })
  // see: cans/05-storage-plane.md#Synthetic-FUSE-inodes
  // see: cans/02-plugin-model.md#FsInodeAttr-carries-full-mode,-uid,-gid,-size,-content
  .onFsLookup((path) => {
    if (escapesWorkspace(path)) {
      throw new Error("E_PATH_TRAVERSAL: lookup escapes the /workspace root");
    }
    if (path !== IDENTITY_PATH) return Verdict.Pass;
    return Verdict.Patch({
      mode: 0o400, uid: 0, gid: 0,
      size: IDENTITY_CONTENT.length,
      content: IDENTITY_CONTENT,
    });
  })
  // see: cans/05-storage-plane.md#In-flight-read-masking
  .onFsRead((_path, buffer) => {
    const window = hookState.tail + Buffer.from(buffer).toString("latin1");
    const masked = maskTokens(window);
    const chunk = masked.slice(masked.length - buffer.length);
    hookState.tail = masked.slice(-CARRYOVER);
    return Buffer.from(chunk, "latin1");
  })
  .onFsWrite((_path, buffer, _pid, _ctx) => {
    return FilterAction.Forward;
  })
  // see: cans/04-terminal-plane.md#1.-ANSI-strip:-drops-OSC-52-(clipboard-hijack)-and-strips-OSC-0/1/2-(title-spoof)
  .onConsoleInput(async (_port, bytes) => {
    const text = Buffer.from(bytes).toString("latin1");
    const stripped = text.replace(/\x1b\]52;[^\x07\x1b]*(?:\x07|\x1b\\)?/g, "");
    return Buffer.from(stripped, "latin1");
  })
  // see: cans/04-terminal-plane.md#4.-Interposer-dispatch:-fires-ConsoleInterposer::on-console-output-for-in-place-masking
  // TODO: live matrix.hook.test.ts masks console output cross-chunk via consoleTail + safeConsoleEmitLen; SSOT keeps single-chunk masking here, see cans/04-terminal-plane.md#Terminal-pipeline
  .onConsoleOutput(async (_port, chunk) => {
    const masked = maskTokens(Buffer.from(chunk).toString("latin1"));
    return Buffer.from(masked, "latin1");
  })
  // see: cans/08-lifecycle.md#Heartbeat
  .onHeartbeat(async (event) => {
    if (hookState.lastUptimeMs === event.uptimeMs && hookState.heartbeats > 0) {
      hookState.stalls += 1;
    }
    hookState.lastUptimeMs = event.uptimeMs;
    hookState.heartbeats += 1;
  })
  // see: cans/02-plugin-model.md#TrapInterposer:-trap,-process-exit,-pressure,-heartbeat,-port,-signal
  .onProcessExit(async (event) => {
    hookState.exits += 1;
    if (event.exitCode !== 0) hookState.failures += 1;
  });

const runtime = kelir({
  vcpus: 2, memoryMb: 2048, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-hooks.erofs",
  tscLocked: true, enableWatchdog: true,
}).use(hookMatrixPlugin);

async function main() {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  const probe = await session.spawn("cat", [IDENTITY_PATH],
                                    { cwd: "/workspace" });
  console.log(`[kelir] cat identity -> exit=${probe.exitCode}`);

  await new Promise((r) => setTimeout(r, 100));
  console.log(`[kelir] heartbeats=${hookState.heartbeats} ` +
              `stalls=${hookState.stalls} exits=${hookState.exits} ` +
              `failures=${hookState.failures}`);

  await session.terminate();
}

main().catch(console.error);
