// see: cans/03-sdk-architecture.md#TypeScript-SDK

export class EConfig extends Error {
  constructor(message: string) {
    super(message);
    this.name = "EConfig";
  }
}
