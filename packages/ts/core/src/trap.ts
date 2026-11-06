// see: cans/03-sdk-architecture.md#TypeScript-SDK
// Trap-filter builder. TS uses camelCase (binary/withArg); a plain
// string literal is also a valid filter (see KelirBuilder.trap).

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
