// see: cans/12-adapters.md#E2B-compatibility
import { kelir, Session } from "@kelir/core";

export class Sandbox {
  private constructor(private readonly session: Session) {}

  static async create(): Promise<Sandbox> {
    const session = await kelir().boot();
    return new Sandbox(session);
  }

  async close(): Promise<void> {
    await this.session.stop();
  }
}
