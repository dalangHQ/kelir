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
