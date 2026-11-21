// see: cans/12-adapters.md#E2B-compatibility
import { Sandbox as CoreSandbox } from "@kelir/core";

export class Sandbox {
  private constructor(private readonly inner: CoreSandbox) {}

  static async create(): Promise<Sandbox> {
    const inner = await (await CoreSandbox.create()).boot();
    return new Sandbox(inner);
  }

  async close(): Promise<void> {
    await this.inner.stop();
  }
}
