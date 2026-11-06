// Minimal ambient declarations for the node globals the examples use.
// No @types/node is installed and none may be network-installed.
// Covers only actual usage: process.arch, process.getuid(),
// crypto.randomBytes().toString("hex"), crypto.randomUUID(), Buffer.from().
// Script-style .d.ts (no top-level import/export) so `declare module`
// registers as an ambient module rather than an augmentation.
declare const process: {
  readonly arch: string;
  getuid?: () => number;
};

declare const Buffer: any;

declare module "node:crypto" {
  const crypto: {
    randomBytes(size: number): { toString(encoding: string): string };
    randomUUID(): string;
  };
  export default crypto;
}
