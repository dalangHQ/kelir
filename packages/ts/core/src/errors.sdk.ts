// see: cans/03-sdk-architecture.md#Prerequisite-gating-across-languages
// see: cans/03-sdk-architecture.md#Distribution-and-error-invariants

export interface ErrorEnvelope {
  readonly code: string;
  readonly message: string;
  readonly driver?: string;
  readonly ring?: string;
  readonly pid?: number;
  readonly binary?: string;
  readonly errno?: string;
}

export const ECONFIG_CODE = "EConfig";

export function parseErrorEnvelope(message: string): ErrorEnvelope | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(message) as unknown;
  } catch {
    return undefined;
  }
  if (typeof parsed !== "object" || parsed === null) {
    return undefined;
  }
  const candidate = parsed as Record<string, unknown>;
  if (typeof candidate.code !== "string" || typeof candidate.message !== "string") {
    return undefined;
  }
  return {
    code: candidate.code,
    message: candidate.message,
    driver: typeof candidate.driver === "string" ? candidate.driver : undefined,
    ring: typeof candidate.ring === "string" ? candidate.ring : undefined,
    pid: typeof candidate.pid === "number" ? candidate.pid : undefined,
    binary: typeof candidate.binary === "string" ? candidate.binary : undefined,
    errno: typeof candidate.errno === "string" ? candidate.errno : undefined,
  };
}

export class EConfig extends Error {
  readonly code: string;
  readonly driver: string | undefined;
  readonly ring: string | undefined;
  readonly pid: number | undefined;
  readonly binary: string | undefined;
  readonly errno: string | undefined;

  constructor(message: string, envelope: Partial<ErrorEnvelope> = {}) {
    super(message);
    this.name = "EConfig";
    this.code = envelope.code ?? ECONFIG_CODE;
    this.driver = envelope.driver;
    this.ring = envelope.ring;
    this.pid = envelope.pid;
    this.binary = envelope.binary;
    this.errno = envelope.errno;
  }

  static fromNative(error: unknown): EConfig {
    const raw = error instanceof Error ? error.message : String(error);
    const envelope = parseErrorEnvelope(raw);
    return envelope === undefined ? new EConfig(raw) : new EConfig(envelope.message, envelope);
  }
}
