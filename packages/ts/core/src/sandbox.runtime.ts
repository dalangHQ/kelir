// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade

import { KelirBuilder, type KelirConfig } from "./builder.lifecycle.js";
import { Session } from "./session.lifecycle.js";
import type { SuspendReason } from "./verdict.protocol.js";
import { EConfig } from "./errors.sdk.js";
import type { SpawnResult } from "./types.core.js";
import {
  parseProfile,
  profileFormatForPath,
  serializeProfile,
  type ProfileSpec,
  type ProfileFormat,
} from "./profile.schema.js";

import { readFile, writeFile } from "node:fs/promises";

function configFromProfile(profile: ProfileSpec): KelirConfig {
  const { session } = profile;
  if (session === undefined) {
    return {};
  }
  return {
    vcpus: session.compute?.vcpus,
    memoryMb: session.compute?.memory_mb,
    pageSizeKb: session.compute?.page_size_kb,
    tscLocked: session.clock?.tsc_locked,
    pinnedClocksource: session.clock?.pinned_clocksource,
    deterministicClock: session.clock?.deterministic_clock,
    preemptionGraceMs: session.clock?.preemption_grace_ms,
    enableWatchdog: session.watchdog?.enabled,
    kernel: session.artifacts?.kernel_path,
    rootfsErofs: session.artifacts?.rootfs_erofs_path,
  };
}

export interface SandboxOptions {
  readonly config?: KelirConfig;
  readonly detached?: boolean;
}

// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
export class Sandbox {
  private builder: KelirBuilder | null;
  private booted: Session | null;
  private readonly profile: ProfileSpec;
  private readonly configPath: string;
  private readonly detachedFlag: boolean;

  private constructor(
    builder: KelirBuilder,
    profile: ProfileSpec,
    sourcePath: string,
  ) {
    this.builder = builder;
    this.booted = null;
    this.profile = profile;
    this.configPath = sourcePath;
    this.detachedFlag = builder.isDetached();
  }

  static async fromConfig(path: string): Promise<Sandbox> {
    if (
      !path.endsWith(".toml") &&
      !path.endsWith(".yaml") &&
      !path.endsWith(".yml")
    ) {
      throw new EConfig(
        `Sandbox.fromConfig: unsupported profile extension for \`${path}\` (expected .toml, .yaml, or .yml)`,
      );
    }
    const text = await readFile(path, "utf8");
    const format: ProfileFormat = profileFormatForPath(path);
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
    profile: ProfileSpec,
    path: string,
  ): Sandbox {
    const builder = new KelirBuilder(configFromProfile(profile));
    return new Sandbox(builder, profile, path);
  }

  profileRecord(): ProfileSpec {
    return this.profile;
  }

  sourcePath(): string {
    return this.configPath;
  }

  sessionId(): string | undefined {
    return this.profile.session?.id;
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

  // see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
  async suspend(reason: SuspendReason): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.suspend before boot");
    }
    return this.booted.suspend(reason);
  }

  // see: cans/08-lifecycle.md#Suspend-and-resume
  async resume(): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.resume before boot");
    }
    return this.booted.resume();
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
    // TODO (L3): full-fidelity round-trip of arbitrary profile sections once plugin namespace schemas register.
    await writeFile(path, serializeProfile(this.profile, profileFormatForPath(path)));
  }

  async stop(): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.stop before boot");
    }
    return this.booted.stop();
  }

  // see: cans/08-lifecycle.md#Pre-teardown-flush
  async terminate(): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.terminate before boot");
    }
    return this.booted.terminate();
  }
}
