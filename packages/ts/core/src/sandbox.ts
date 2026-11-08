// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade

import { KelirBuilder, type KelirConfig } from "./builder.js";
import { Session } from "./session.js";
import { EConfig } from "./errors.js";
import type { SpawnResult } from "./types.js";

import { readFile, writeFile } from "node:fs/promises";

export type ProfileRecord = Record<string, string | number | boolean>;

function parseScalar(raw: string): string | number | boolean | undefined {
  const text = raw.trim();
  if (text === "true") {
    return true;
  }
  if (text === "false") {
    return false;
  }
  if (/^-?\d+$/.test(text)) {
    return Number(text);
  }
  if (text.startsWith('"') && text.endsWith('"')) {
    return text.slice(1, -1);
  }
  if (text.startsWith("'") && text.endsWith("'")) {
    return text.slice(1, -1);
  }
  return undefined;
}

function stripTomlComment(line: string): string {
  const hash = line.indexOf("#");
  if (hash === -1) {
    return line;
  }
  const quote = line.indexOf('"');
  if (quote !== -1 && quote < hash) {
    return line;
  }
  return line.slice(0, hash);
}

// TODO (L3): full TOML/YAML grammar; this subset reads only the scalar session-compute keys the profile fixtures declare.
export function parseTomlProfile(text: string): ProfileRecord {
  const record: ProfileRecord = {};
  let section = "";
  for (const rawLine of text.split("\n")) {
    const line = stripTomlComment(rawLine).trim();
    if (line === "") {
      continue;
    }
    const sectionMatch = /^\[([^\]]+)\]$/.exec(line);
    if (sectionMatch !== null) {
      section = sectionMatch[1].trim();
      continue;
    }
    const kvMatch = /^([A-Za-z0-9_-]+)\s*=\s*(.+)$/.exec(line);
    if (kvMatch === null) {
      continue;
    }
    const value = parseScalar(kvMatch[2]);
    if (value === undefined) {
      continue;
    }
    record[section === "" ? kvMatch[1] : `${section}.${kvMatch[1]}`] = value;
  }
  return record;
}

// TODO (L3): full TOML/YAML grammar; this subset reads only the scalar session-compute keys the profile fixtures declare.
export function parseYamlProfile(text: string): ProfileRecord {
  const record: ProfileRecord = {};
  const stack: { indent: number; name: string }[] = [];
  for (const rawLine of text.split("\n")) {
    const trimmed = rawLine.trim();
    if (trimmed === "" || trimmed.startsWith("#") || trimmed.startsWith("-")) {
      continue;
    }
    const indent = rawLine.length - rawLine.trimStart().length;
    const kvMatch = /^([A-Za-z0-9_-]+):\s*(.*)$/.exec(trimmed);
    if (kvMatch === null) {
      continue;
    }
    const key = kvMatch[1];
    const rest = kvMatch[2].trim();
    while (stack.length > 0 && indent <= stack[stack.length - 1].indent) {
      stack.pop();
    }
    if (rest === "") {
      stack.push({ indent, name: key });
      continue;
    }
    const comment = rest.indexOf(" #");
    const bare = (comment === -1 ? rest : rest.slice(0, comment)).trim();
    const value = parseScalar(bare);
    if (value === undefined) {
      continue;
    }
    const path = [...stack.map((entry) => entry.name), key].join(".");
    record[path] = value;
  }
  return record;
}

export function profileToToml(record: ProfileRecord): string {
  const sections = new Map<string, [string, string | number | boolean][]>();
  for (const [key, value] of Object.entries(record)) {
    const dot = key.lastIndexOf(".");
    const section = dot === -1 ? "" : key.slice(0, dot);
    const leaf = dot === -1 ? key : key.slice(dot + 1);
    const entries = sections.get(section) ?? [];
    entries.push([leaf, value]);
    sections.set(section, entries);
  }
  const lines: string[] = [];
  for (const [section, entries] of sections) {
    if (section !== "") {
      lines.push(`[${section}]`);
    }
    for (const [leaf, value] of entries) {
      const rendered =
        typeof value === "string" ? `"${value}"` : String(value);
      lines.push(`${leaf} = ${rendered}`);
    }
    lines.push("");
  }
  return lines.join("\n");
}

export interface SandboxOptions {
  readonly config?: KelirConfig;
  readonly detached?: boolean;
}

// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
export class Sandbox {
  private builder: KelirBuilder | null;
  private booted: Session | null;
  private readonly profile: ProfileRecord;
  private readonly configPath: string;
  private readonly detachedFlag: boolean;

  private constructor(
    builder: KelirBuilder,
    profile: ProfileRecord,
    sourcePath: string,
  ) {
    this.builder = builder;
    this.booted = null;
    this.profile = profile;
    this.configPath = sourcePath;
    this.detachedFlag = builder.isDetached();
  }

  static async fromConfig(path: string): Promise<Sandbox> {
    const text = await readFile(path, "utf8");
    let profile: ProfileRecord;
    if (path.endsWith(".toml")) {
      profile = parseTomlProfile(text);
    } else if (path.endsWith(".yaml") || path.endsWith(".yml")) {
      profile = parseYamlProfile(text);
    } else {
      throw new EConfig(
        `Sandbox.fromConfig: unsupported profile extension for \`${path}\` (expected .toml, .yaml, or .yml)`,
      );
    }
    return Sandbox.fromProfile(profile, path);
  }

  static async create(opts: SandboxOptions = {}): Promise<Sandbox> {
    const builder = new KelirBuilder(opts.config ?? {});
    if (opts.detached !== undefined) {
      builder.detached(opts.detached);
    }
    return new Sandbox(builder, {}, "");
  }

  private static fromProfile(
    profile: ProfileRecord,
    path: string,
  ): Sandbox {
    const config: KelirConfig = {
      vcpus:
        typeof profile["session.compute.vcpus"] === "number"
          ? profile["session.compute.vcpus"]
          : undefined,
      memoryMb:
        typeof profile["session.compute.memory_mb"] === "number"
          ? profile["session.compute.memory_mb"]
          : undefined,
      kernel:
        typeof profile["session.artifacts.kernel_path"] === "string"
          ? profile["session.artifacts.kernel_path"]
          : undefined,
      rootfsErofs:
        typeof profile["session.artifacts.rootfs_erofs_path"] === "string"
          ? profile["session.artifacts.rootfs_erofs_path"]
          : undefined,
      tscLocked:
        typeof profile["session.clock.tsc_locked"] === "boolean"
          ? profile["session.clock.tsc_locked"]
          : undefined,
      enableWatchdog:
        typeof profile["session.watchdog.enabled"] === "boolean"
          ? profile["session.watchdog.enabled"]
          : undefined,
    };
    const builder = new KelirBuilder(config);
    return new Sandbox(builder, profile, path);
  }

  profileRecord(): ProfileRecord {
    return this.profile;
  }

  sourcePath(): string {
    return this.configPath;
  }

  sessionId(): string | undefined {
    const id = this.profile["session.id"];
    return typeof id === "string" ? id : undefined;
  }

  session(): Session | null {
    return this.booted;
  }

  isDetached(): boolean {
    return this.detachedFlag;
  }

  async boot(): Promise<this> {
    if (this.builder === null) {
      throw new EConfig("Sandbox.boot called twice");
    }
    const builder = this.builder;
    this.builder = null;
    this.booted = await builder.boot();
    return this;
  }

  async exec(binary: string, argv: string[] = []): Promise<SpawnResult> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.exec before boot");
    }
    return this.booted.spawn(binary, argv);
  }

  async rollback(): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.rollback before boot");
    }
    return this.booted.rollback();
  }

  async suspend(path: string): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.suspend before boot");
    }
    return this.booted.suspend(path);
  }

  async reconcile(options: {
    cwd: string;
    atomicStaging?: boolean;
    fsync?: boolean;
  }): Promise<{ filesChanged: number; tombstonesResolved: number }> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.reconcile before boot");
    }
    return this.booted.storage.reconcile(options);
  }

  async exportSpec(path: string): Promise<void> {
    await writeFile(path, profileToToml(this.profile));
  }

  async stop(): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.stop before boot");
    }
    return this.booted.stop();
  }

  async terminate(): Promise<void> {
    return this.stop();
  }
}
