// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Filter-before-FFI-gate

import type { TrapFilterSnapshot } from "./fusion.plugin.js";

export class TrapFilter {
  private readonly binaryName?: string;
  private readonly argMatchers: readonly string[];
  private readonly globPattern?: string;
  private readonly cwdPattern?: string;
  private readonly envMatchers: readonly (readonly [string, string])[];
  private readonly uidFilter?: number;
  private readonly argvPattern?: string;

  private constructor(
    binaryName: string | undefined,
    argMatchers: readonly string[],
    globPattern: string | undefined,
    cwdPattern: string | undefined,
    envMatchers: readonly (readonly [string, string])[],
    uidFilter: number | undefined,
    argvPattern: string | undefined,
  ) {
    this.binaryName = binaryName;
    this.argMatchers = argMatchers;
    this.globPattern = globPattern;
    this.cwdPattern = cwdPattern;
    this.envMatchers = envMatchers;
    this.uidFilter = uidFilter;
    this.argvPattern = argvPattern;
  }

  static binary(name: string): TrapFilter {
    return new TrapFilter(name, [], undefined, undefined, [], undefined, undefined);
  }

  static pattern(glob: string): TrapFilter {
    return new TrapFilter(undefined, [], glob, undefined, [], undefined, undefined);
  }

  withArg(arg: string): TrapFilter {
    return new TrapFilter(
      this.binaryName,
      [...this.argMatchers, arg],
      this.globPattern,
      this.cwdPattern,
      this.envMatchers,
      this.uidFilter,
      this.argvPattern,
    );
  }

  withCwd(cwd: string): TrapFilter {
    return new TrapFilter(
      this.binaryName,
      this.argMatchers,
      this.globPattern,
      cwd,
      this.envMatchers,
      this.uidFilter,
      this.argvPattern,
    );
  }

  withEnv(key: string, value: string): TrapFilter {
    return new TrapFilter(
      this.binaryName,
      this.argMatchers,
      this.globPattern,
      this.cwdPattern,
      [...this.envMatchers, [key, value] as const],
      this.uidFilter,
      this.argvPattern,
    );
  }

  withUid(uid: number): TrapFilter {
    return new TrapFilter(
      this.binaryName,
      this.argMatchers,
      this.globPattern,
      this.cwdPattern,
      this.envMatchers,
      uid,
      this.argvPattern,
    );
  }

  argvMatches(pattern: string): TrapFilter {
    return new TrapFilter(
      this.binaryName,
      this.argMatchers,
      this.globPattern,
      this.cwdPattern,
      this.envMatchers,
      this.uidFilter,
      pattern,
    );
  }

  snapshot(): TrapFilterSnapshot {
    return {
      binary: this.binaryName,
      args: this.argMatchers,
      pattern: this.globPattern,
      cwd: this.cwdPattern,
      env: this.envMatchers,
      uid: this.uidFilter,
      argv: this.argvPattern,
    };
  }
}
