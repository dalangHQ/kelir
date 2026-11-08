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
  readonly otel?: {
    readonly endpoint?: string;
    readonly traceParent: boolean;
    readonly serviceName: string;
  };
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

export interface ContainerConfig extends PluginEnablement {
  readonly runtime?: string;
  readonly isolateVarLibContainers?: boolean;
  readonly pinSubuidRanges?: boolean;
}

export function container(config: ContainerConfig): KelirPlugin {
  return { name: "container", enabled: config.enabled, config };
}

export interface DbProxyConfig extends PluginEnablement {
  readonly postgres?: {
    readonly listenPort?: number;
    readonly upstream?: string;
    readonly rewriteRules?: readonly { readonly from: string; readonly to: string }[];
    readonly maxRewritesPerConn?: number;
  };
  readonly sqlite?: { readonly guestSocket?: string; readonly hostPath?: string };
}

export function dbProxy(config: DbProxyConfig): KelirPlugin {
  return { name: "db-proxy", enabled: config.enabled, config };
}

export interface MeshConfig extends PluginEnablement {
  readonly peerIdentity?: { readonly trustDomain: string; readonly workloadPrefix: string };
  readonly mtls?: boolean;
  readonly propagateTraceparent?: boolean;
  readonly listeners?: readonly { readonly port: number; readonly service: string }[];
}

export function mesh(config: MeshConfig): KelirPlugin {
  return { name: "mesh", enabled: config.enabled, config };
}

export interface ProfileConfig extends PluginEnablement {
  readonly rfc6902DynamicPatching?: boolean;
  readonly enforceMonotonicPatches?: boolean;
  readonly cdi?: { readonly enabled?: boolean; readonly specDir?: string };
}

export function profile(config: ProfileConfig): KelirPlugin {
  return { name: "profile", enabled: config.enabled, config };
}

export interface ReplayConfig extends PluginEnablement {
  readonly stepWindow?: number;
  readonly kernelParams?: readonly string[];
  readonly deterministicRngSeed?: number;
  readonly networkIsolation?: boolean;
}

export function replay(config: ReplayConfig): KelirPlugin {
  return { name: "replay", enabled: config.enabled, config };
}

export interface SupplyChainConfig extends PluginEnablement {
  readonly wxJit?: {
    readonly trapMemfd?: boolean;
    readonly trapMprotectExec?: boolean;
    readonly allowedJitBinaries?: readonly string[];
  };
  readonly packageGuard?: {
    readonly denyLifecycleScripts?: boolean;
    readonly managers?: readonly string[];
    readonly shannonEntropyThresholdBits?: number;
    readonly bypassRunners?: readonly string[];
  };
}

export function supplyChain(config: SupplyChainConfig): KelirPlugin {
  return { name: "supply-chain", enabled: config.enabled, config };
}

export interface TransactionConfig extends PluginEnablement {
  readonly timeoutMs?: number;
  readonly autoRollbackOnTaint?: boolean;
  readonly flushFuseBeforeCommit?: boolean;
}

export function transaction(config: TransactionConfig): KelirPlugin {
  return { name: "transaction", enabled: config.enabled, config };
}

export interface VaultConfig extends PluginEnablement {
  readonly metadataVsockPort?: number;
  readonly tokenTtlSeconds?: number;
}

export function vault(config: VaultConfig): KelirPlugin {
  return { name: "vault", enabled: config.enabled, config };
}
