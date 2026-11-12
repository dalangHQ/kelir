// see: cans/02-plugin-model.md#Segregated-ring-traits

import { kelir, createPlugin, Verdict, FilterAction, SuspendUntil,
         type Session } from "../dist/index.js";


const IDENTITY_PATH = "/workspace/.policy/identity";
const IDENTITY_CONTENT = "policy-matrix-identity\n";
const DENY_MARKER = "198.51.100.";
const TOKEN_PREFIX = "sk_live_";
const TOKEN_LEN = TOKEN_PREFIX.length + 24;


const CARRYOVER = TOKEN_LEN - 1;


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
  consoleTail: "" as string,
  heartbeats: 0,
  lastUptimeMs: null as number | null,
  stalls: 0,
  exits: 0,
  failures: 0,
};


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

// see: cans/04-terminal-plane.md#Terminal-pipeline
function safeConsoleEmitLen(window: Uint8Array): number {
  const emit = Math.max(0, window.length - CARRYOVER);
  return backOffIncompleteAnsi(window, backOffIncompleteUtf8(window, emit));
}

function backOffIncompleteUtf8(window: Uint8Array, emit: number): number {
  if (emit === 0 || emit >= window.length) return emit;
  let start = emit;
  let continuations = 0;
  while (start > 0 && continuations < 3 && window[start - 1] >= 0x80 && window[start - 1] < 0xC0) {
    start -= 1;
    continuations += 1;
  }
  if (start === 0) return emit;
  const lead = window[start - 1];
  let needed = 0;
  if (lead >= 0xC2 && lead < 0xE0) needed = 2;
  else if (lead >= 0xE0 && lead < 0xF0) needed = 3;
  else if (lead >= 0xF0 && lead < 0xF5) needed = 4;
  else return emit;
  return start - 1 + needed > emit ? start - 1 : emit;
}

function backOffIncompleteAnsi(window: Uint8Array, emit: number): number {
  if (emit === 0) return emit;
  let escape = -1;
  for (let i = 0; i < emit; i++) {
    if (window[i] === 0x1b) escape = i;
  }
  if (escape === -1) return emit;
  if (escape + 1 >= emit) return escape;
  if (window[escape + 1] === 0x5b) {
    for (let i = escape + 2; i < emit; i++) {
      if (window[i] >= 0x40 && window[i] < 0x7f) return emit;
    }
    return escape;
  }
  if (window[escape + 1] === 0x5d) {
    for (let i = escape + 2; i < emit; i++) {
      if (window[i] === 0x07) return emit;
      if (window[i] === 0x1b && window[i + 1] === 0x5c) return emit;
    }
    return escape;
  }
  return emit;
}

const hookMatrixPlugin = createPlugin<"hook-matrix", Record<string, never>>("hook-matrix")


  .onNetOutbound((frame) => {
    const text = Buffer.from(frame).toString("latin1");
    if (text.includes(DENY_MARKER)) {

      return FilterAction.Suspend(SuspendUntil.deadline(0));
    }
    return FilterAction.Forward;
  })


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


  .onConsoleInput(async (_port, bytes) => {
    const text = Buffer.from(bytes).toString("latin1");
    const stripped = text.replace(/\x1b\]52;[^\x07\x1b]*(?:\x07|\x1b\\)?/g, "");
    return Buffer.from(stripped, "latin1");
  })


  .onConsoleOutput(async (_port, chunk) => {
    const window = Buffer.concat([
      Buffer.from(hookState.consoleTail, "latin1"),
      Buffer.from(chunk),
    ]);
    const masked = Buffer.from(maskTokens(window.toString("latin1")), "latin1");
    const emit = safeConsoleEmitLen(masked);
    hookState.consoleTail = masked.subarray(emit).toString("latin1");
    return Buffer.from(masked.subarray(0, emit));
  })


  .onHeartbeat(async (event) => {
    if (hookState.lastUptimeMs === event.uptimeMs && hookState.heartbeats > 0) {
      hookState.stalls += 1;
    }
    hookState.lastUptimeMs = event.uptimeMs;
    hookState.heartbeats += 1;
  })

  .onProcessExit(async (event) => {
    hookState.exits += 1;
    if (event.exitCode !== 0) hookState.failures += 1;
  })


const runtime = kelir({
  vcpus: 2, memoryMb: 2048, driver: "auto",
  kernel: `./fixtures/vmlinux-6.6-${process.arch}-kelir`,
  rootfsErofs: "./fixtures/rootfs-hooks.erofs",
  tscLocked: true, enableWatchdog: true,
}).use(hookMatrixPlugin);

async function hookMatrixCase_identityPatchProbe(session: Session): Promise<void> {
  const probe = await session.spawn("cat", [IDENTITY_PATH],
                                    { cwd: "/workspace" });
  console.log(`[kelir] cat identity -> exit=${probe.exitCode}`);
}

async function hookMatrixCase_heartbeatAndExitCounters(): Promise<void> {
  await new Promise((r) => setTimeout(r, 100));
  console.log(`[kelir] heartbeats=${hookState.heartbeats} ` +
              `stalls=${hookState.stalls} exits=${hookState.exits} ` +
              `failures=${hookState.failures}`);
}

async function main(): Promise<void> {
  const session: Session = await runtime.boot();
  console.log(`[kelir] driver=${session.driverName} boot=${session.bootMs}ms`);

  await hookMatrixCase_identityPatchProbe(session);
  await hookMatrixCase_heartbeatAndExitCounters();

  await session.terminate();
}

main().catch(console.error);
