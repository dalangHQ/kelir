// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade

import { KelirBuilder, type KelirConfig } from "./builder.lifecycle.flow.js";

type HostBuilder = KelirBuilder<"host", "unsealed", string, unknown, never, unknown>;
import { Session } from "./session.lifecycle.flow.js";
import type { SuspendReason } from "./verdict.protocol.flow.js";
import { EConfig } from "./errors.sdk.flow.js";
import type { SnapshotDescriptor, SpawnResult } from "./types.core.flow.js";
import {
  configFromProfile,
  profileFormatForPath,
  readProfile,
  serializeProfile,
  type ProfileSpec,
} from "./profile.schema.flow.js";
import { writeProfileText } from "./profile.schema.run.js";

export interface SandboxOptions {
  readonly config?: KelirConfig;
  readonly detached?: boolean;
}

// see: cans/03-sdk-architecture.md#Sandbox-high-level-facade
export class Sandbox {
  private builder: HostBuilder | null;
  private booted: Session | null;
  private readonly profile: ProfileSpec;
  private readonly configPath: string;
  private readonly detachedFlag: boolean;

  private constructor(
    builder: HostBuilder,
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
    return Sandbox.fromProfile(await readProfile(path), path);
  }

  static async create(opts: SandboxOptions = {}): Promise<Sandbox> {
    const builder: HostBuilder = new KelirBuilder<"host", "unsealed", string, unknown, never, unknown>(opts.config ?? {});
    if (opts.detached !== undefined) {
      builder.detached(opts.detached);
    }
    return new Sandbox(builder, {}, "");
  }

  private static fromProfile(
    profile: ProfileSpec,
    path: string,
  ): Sandbox {
    const builder: HostBuilder = new KelirBuilder<"host", "unsealed", string, unknown, never, unknown>(configFromProfile(profile));
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

  async rollback(snapshot?: SnapshotDescriptor): Promise<void> {
    if (this.booted === null) {
      throw new EConfig("Sandbox.rollback before boot");
    }
    return this.booted.rollback(snapshot);
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
    // L3-GAP: [bindings_ts]: full-fidelity round-trip of arbitrary profile sections once plugin namespace schemas register.; slop: profile sections lost in round-trip, namespace silently dropped
    // TODO (L2) [profile_ingestion]: overlay programmatic plane descriptors over the ingested profile before export.; slop: post-ingestion descriptor overrides silently absent from the export
    await writeProfileText(
      path,
      serializeProfile(this.profile, profileFormatForPath(path)),
    );
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
