// see: cans/03-sdk-architecture.md#TypeScript-SDK
declare module "node:fs/promises" {
  function readFile(path: string, encoding: "utf8"): Promise<string>;
  function writeFile(path: string, data: string): Promise<void>;
}
