// see: cans/03-sdk-architecture.md#TypeScript-SDK
declare module "node:fs/promises" {
  function readFile(path: string, encoding: "utf8"): Promise<string>;
  function writeFile(path: string, data: string): Promise<void>;
}
// see: cans/02-plugin-model.md#Read/console-pin-(SSOT-A1)
declare module "node:buffer" {
  export interface Buffer extends Uint8Array {
    toString(encoding?: string, start?: number, end?: number): string;
  }
  export function from(value: string, encoding?: string): Buffer;
}
