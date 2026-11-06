// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface KelirPlugin {
  readonly name: string;
}

export class Session {
  // see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
  async rollback(): Promise<void> {
    // TODO: Call native session rollback
  }

  async stop(): Promise<void> {
    // TODO: Call native session stop
  }
}

export class KelirBuilder {
  private plugins: KelirPlugin[] = [];

  use<P extends KelirPlugin>(plugin: P): this {
    this.plugins.push(plugin);
    return this;
  }

  async boot(): Promise<Session> {
    // TODO: Cross NAPI boundary to NativeKelirBuilder.boot()
    return new Session();
  }
}

export function kelir(): KelirBuilder {
  return new KelirBuilder();
}
