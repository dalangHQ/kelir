// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade

import { KelirBuilder, type KelirConfig } from "./builder.lifecycle.js";
import { Session } from "./session.lifecycle.js";
import { EConfig } from "./errors.js";
import type { SpawnResult } from "./types.js";

import { readFile, writeFile } from "node:fs/promises";
import { parse as parseToml } from "@ltd/j-toml";
import { parse as parseYaml } from "yaml";

export type ProfileValue =
  | string
  | number
  | boolean
  | ProfileValue[]
  | ProfileRecord;

export type ProfileRecord = { readonly [key: string]: ProfileValue };

export type ProfileFormat = "toml" | "yaml";

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
interface ProfileField {
  readonly path: readonly string[];
  readonly kind: "boolean" | "integer" | "string";
  readonly configKey?: keyof KelirConfig;
}

// see: cans/plugins/profile.md#Invariants-and-policy-(profile)
const PROFILE_FIELDS: readonly ProfileField[] = [
  { path: ["session", "id"], kind: "string" },
  { path: ["session", "compute", "vcpus"], kind: "integer", configKey: "vcpus" },
  { path: ["session", "compute", "memory_mb"], kind: "integer", configKey: "memoryMb" },
  { path: ["session", "artifacts", "kernel_path"], kind: "string", configKey: "kernel" },
  {
    path: ["session", "artifacts", "rootfs_erofs_path"],
    kind: "string",
    configKey: "rootfsErofs",
  },
  { path: ["session", "clock", "tsc_locked"], kind: "boolean", configKey: "tscLocked" },
  { path: ["session", "watchdog", "enabled"], kind: "boolean", configKey: "enableWatchdog" },
];

export function parseProfile(text: string, format: ProfileFormat): ProfileRecord {
  const loaded: unknown =
    format === "toml" ? parseToml(text, { bigint: false }) : parseYaml(text);
  if (typeof loaded === "object" && loaded !== null && !Array.isArray(loaded)) {
    return loaded as ProfileRecord;
  }
  return {};
}

function profileValueAt(
  profile: ProfileRecord,
  path: readonly string[],
): ProfileValue | undefined {
  let node: ProfileValue | undefined = profile;
  for (const key of path) {
    if (node === undefined || typeof node !== "object" || Array.isArray(node)) {
      return undefined;
    }
    node = node[key];
  }
  return node;
}

function matchesField(
  value: ProfileValue | undefined,
  kind: ProfileField["kind"],
): boolean {
  if (kind === "boolean") {
    return typeof value === "boolean";
  }
  if (kind === "integer") {
    return typeof value === "number" && Number.isInteger(value);
  }
  return typeof value === "string";
}

// TODO (L3): schema-validate every profile section and reject ill-typed values with structured EConfig.
function configFromProfile(profile: ProfileRecord): KelirConfig {
  const config: KelirConfig = {};
  for (const field of PROFILE_FIELDS) {
    const value = profileValueAt(profile, field.path);
    if (field.configKey !== undefined && matchesField(value, field.kind)) {
      (config as Record<string, unknown>)[field.configKey] = value;
    }
  }
  return config;
}

function renderTomlValue(kind: ProfileField["kind"], value: ProfileValue): string {
  if (kind === "boolean") {
    return value ? "true" : "false";
  }
  if (kind === "integer") {
    return String(value);
  }
  return JSON.stringify(value);
}

function renderToml(profile: ProfileRecord): string {
  const lines: string[] = [];
  let section: string | undefined;
  for (const field of PROFILE_FIELDS) {
    const value = profileValueAt(profile, field.path);
    if (value === undefined || !matchesField(value, field.kind)) {
      continue;
    }
    const head = field.path.slice(0, -1).join(".");
    if (head !== section) {
      if (lines.length > 0) {
        lines.push("");
      }
      lines.push(`[${head}]`);
      section = head;
    }
    lines.push(
      `${field.path[field.path.length - 1]} = ${renderTomlValue(field.kind, value)}`,
    );
  }
  return lines.length === 0 ? "" : `${lines.join("\n")}\n`;
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
    let format: ProfileFormat;
    if (path.endsWith(".toml")) {
      format = "toml";
    } else if (path.endsWith(".yaml") || path.endsWith(".yml")) {
      format = "yaml";
    } else {
      throw new EConfig(
        `Sandbox.fromConfig: unsupported profile extension for \`${path}\` (expected .toml, .yaml, or .yml)`,
      );
    }
    return Sandbox.fromProfile(parseProfile(text, format), path);
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
    const builder = new KelirBuilder(configFromProfile(profile));
    return new Sandbox(builder, profile, path);
  }

  profileRecord(): ProfileRecord {
    return this.profile;
  }

  sourcePath(): string {
    return this.configPath;
  }

  sessionId(): string | undefined {
    const id = profileValueAt(this.profile, ["session", "id"]);
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
    // TODO (L3): full-fidelity round-trip of arbitrary profile sections via a declared TOML writer.
    await writeFile(path, renderToml(this.profile));
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
