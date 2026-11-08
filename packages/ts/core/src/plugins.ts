// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { KelirPlugin } from "./plugin.js";

export interface PluginEnablement {
  readonly enabled?: boolean;
}

export interface ChaosConfig extends PluginEnablement {
  readonly network?: readonly {
    readonly target: string;
    readonly latency: { readonly baseMs: number; readonly jitterMs: number };
    readonly dropRate: number;
    readonly direction?: "inbound" | "outbound" | "both";
  }[];
  readonly fs?: readonly {
    readonly targetPath: string;
    readonly simulateError: string;
    readonly errorProbability: number;
  }[];
}

export function chaos(config: ChaosConfig): KelirPlugin {
  return { name: "chaos", enabled: config.enabled, config };
}

export interface LedgerConfig extends PluginEnablement {
  readonly store?: string;
  readonly merkleChain?: boolean;
  readonly otel?: { readonly traceParent: boolean; readonly serviceName: string };
  readonly receipts?: {
    readonly includeDirtyBytes?: boolean;
    readonly includeBinaryBlake3?: boolean;
    readonly includeSyscallCount?: boolean;
  };
}

export function ledger(config: LedgerConfig): KelirPlugin {
  return { name: "ledger", enabled: config.enabled, config };
}

export interface McpConfig extends PluginEnablement {
  readonly transport: {
    readonly streamKind: string;
    readonly framing: string;
  };
  readonly advertise: { readonly tools: readonly string[] };
}

export function mcp(config: McpConfig): KelirPlugin {
  return { name: "mcp", enabled: config.enabled, config };
}

export interface ToolGuardConfig extends PluginEnablement {
  readonly pinSchemas: boolean;
  readonly maxInvocationsPerTrace: number;
  readonly approvals: Record<string, { readonly action: string; readonly durationMs: number }>;
}

export function toolGuard(config: ToolGuardConfig): KelirPlugin {
  return { name: "tool-guard", enabled: config.enabled, config };
}

export interface WireIngressConfig extends PluginEnablement {
  readonly listen: { readonly port: number; readonly protocol: string };
  readonly parking: { readonly maxBufferedBytes: number; readonly idleTimeoutMs: number };
  readonly wakeOnPacket: { readonly transportChannel: number };
  readonly backlog: { readonly maxConnections: number; readonly overflow: string };
}

export function wireIngress(config: WireIngressConfig): KelirPlugin {
  return { name: "wire-ingress", enabled: config.enabled, config };
}
