// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/02-plugin-model.md#Filter-before-FFI-gate

import type { TrapFilterSnapshot } from "./fusion.js";

export class TrapFilter {
  private readonly binaryName?: string;
  private readonly argMatchers: readonly string[];
  private readonly globPattern?: string;

  private constructor(
    binaryName: string | undefined,
    argMatchers: readonly string[],
    globPattern: string | undefined,
  ) {
    this.binaryName = binaryName;
    this.argMatchers = argMatchers;
    this.globPattern = globPattern;
  }

  static binary(name: string): TrapFilter {
    return new TrapFilter(name, [], undefined);
  }

  static pattern(glob: string): TrapFilter {
    return new TrapFilter(undefined, [], glob);
  }

  withArg(arg: string): TrapFilter {
    return new TrapFilter(this.binaryName, [...this.argMatchers, arg], this.globPattern);
  }

  snapshot(): TrapFilterSnapshot {
    return {
      binary: this.binaryName,
      args: this.argMatchers,
      pattern: this.globPattern,
    };
  }
}
