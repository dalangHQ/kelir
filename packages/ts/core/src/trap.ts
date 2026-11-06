// see: cans/03-sdk-architecture.md#TypeScript-SDK

export class TrapFilter {
  static binary(name: string): TrapFilter {
    void name;
    return new TrapFilter();
  }

  withArg(arg: string): TrapFilter {
    void arg;
    return this;
  }
}
