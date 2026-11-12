// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { ChaosHandle, LedgerHandle, MeshHandle } from "./context.lifecycle.types.js";
import { createPlugin } from "./builder.lifecycle.js";
import type { KelirBuilder } from "./builder.lifecycle.js";

export interface PluginEnablement {
  readonly enabled?: boolean;
}


export interface NetworkFaultRule {
  readonly target?: string;
  readonly direction?: "outbound" | "inbound" | "both";
  readonly baseMs?: number;
  readonly jitterMs?: number;
  readonly dropRate?: number;
  readonly injectRst?: boolean;
  readonly bandwidthKbps?: number;
}


export interface StorageFaultRule {
  readonly targetPath?: string;
  readonly simulateError?: "eperm" | "eacces" | "enoent" | "einval" | "enospc" | "erofs" | "econnrefused" | "enxio" | "ewouldblock" | "efault" | "ebusy" | "exdev" | "epipe" | "enodata" | "eproto" | "epolicy" | "etoolquota" | "ewxviolation" | "econfig" | "esecurityviolation" | "eunknownkind" | "eclockregress";
  readonly errorProbability?: number;
}


export interface ChaosConfig extends PluginEnablement {
  readonly network?: readonly NetworkFaultRule[];
  readonly fs?: readonly StorageFaultRule[];
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function chaos<P = unknown>(config: ChaosConfig): KelirBuilder<"chaos", { chaos: ChaosHandle }, never, P> {
  return createPlugin<"chaos", { chaos: ChaosHandle }, ChaosConfig, never, P>("chaos", { enabled: config.enabled, config });
}


export interface ContainerConfig extends PluginEnablement {
  readonly runtime?: string;
  readonly isolateVarLibContainers?: boolean;
  readonly allowUnsandboxed?: boolean;
  readonly pinSubuidRanges?: boolean;
  readonly scratchDevice?: string;
}


export function container<P = unknown>(config: ContainerConfig): KelirBuilder<"container", unknown, never, P> {
  return createPlugin<"container", unknown, ContainerConfig, never, P>("container", { enabled: config.enabled, config });
}


export interface PgWireRewrite {
  readonly listenPort?: number;
  readonly upstream?: string;
  readonly rewriteRules?: readonly ([string, string])[];
  readonly maxRewritesPerConn?: number;
}


export interface SqliteOverVsock {
  readonly guestSocket?: string;
  readonly hostPath?: string;
}


export interface EphemeralCredentials {
  readonly placeholder?: string;
  readonly ttlSeconds?: number;
}


export interface DbProxyConfig extends PluginEnablement {
  readonly postgres?: PgWireRewrite;
  readonly sqlite?: SqliteOverVsock;
  readonly credentials?: EphemeralCredentials;
}


export function dbProxy<P = unknown>(config: DbProxyConfig): KelirBuilder<"db-proxy", unknown, never, P> {
  return createPlugin<"db-proxy", unknown, DbProxyConfig, never, P>("db-proxy", { enabled: config.enabled, config });
}


export interface OtelConfig {
  readonly endpoint?: string;
  readonly serviceName?: string;
  readonly propagateW3cTraceparent?: boolean;
  readonly exportDiagnosticPmuGauges?: boolean;
}


export interface ReceiptConfig {
  readonly includeDirtyBytes?: boolean;
  readonly includeBinaryBlake3?: boolean;
  readonly includeSyscallCount?: boolean;
  readonly includeWallClockNs?: boolean;
  readonly includePhaseTimings?: boolean;
}


export interface LedgerConfig extends PluginEnablement {
  readonly store?: string;
  readonly merkleChain?: boolean;
  readonly suppressLocalDb?: boolean;
  readonly otel?: OtelConfig;
  readonly receipts?: ReceiptConfig;
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function ledger<P = unknown>(config: LedgerConfig): KelirBuilder<"ledger", { ledger: LedgerHandle }, never, P> {
  return createPlugin<"ledger", { ledger: LedgerHandle }, LedgerConfig, never, P>("ledger", { enabled: config.enabled, config });
}


export interface McpConfig extends PluginEnablement {
}


export function mcp<P = unknown>(config: McpConfig): KelirBuilder<"mcp", unknown, never, P> {
  return createPlugin<"mcp", unknown, McpConfig, never, P>("mcp", { enabled: config.enabled, config });
}


export interface PeerListener {
  readonly port?: number;
  readonly service?: string;
}


export interface PeerIdentity {
  readonly trustDomain?: string;
  readonly workloadPrefix?: string;
}


export interface MeshConfig extends PluginEnablement {
  readonly peerIdentity?: PeerIdentity;
  readonly mtls?: boolean;
  readonly traceparent?: "inject-and-validate";
  readonly listeners?: readonly PeerListener[];
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function mesh<P = unknown>(config: MeshConfig): KelirBuilder<"mesh", { mesh: MeshHandle }, never, P> {
  return createPlugin<"mesh", { mesh: MeshHandle }, MeshConfig, never, P>("mesh", { enabled: config.enabled, config });
}


export interface CdiConfig {
  readonly enabled?: boolean;
  readonly specDir?: string;
}


export interface ProfileConfig extends PluginEnablement {
  readonly rfc6902DynamicPatching?: boolean;
  readonly enforceMonotonicPatches?: boolean;
  readonly cdi?: CdiConfig;
}


export function profile<P = unknown>(config: ProfileConfig): KelirBuilder<"profile", unknown, never, P> {
  return createPlugin<"profile", unknown, ProfileConfig, never, P>("profile", { enabled: config.enabled, config });
}


export interface ReplayConfig extends PluginEnablement {
  readonly stepWindow?: number;
  readonly kernelParams?: readonly string[];
  readonly deterministicRngSeed?: number;
  readonly networkIsolation?: boolean;
}


export function replay<P = unknown>(config: ReplayConfig): KelirBuilder<"replay", unknown, never, P> {
  return createPlugin<"replay", unknown, ReplayConfig, never, P>("replay", { enabled: config.enabled, config });
}


export interface AuthorizedJitEngine {
  readonly binaryPath?: string;
  readonly binaryBlake3?: Uint8Array;
}


export interface WxJitConfig {
  readonly trapMemfd?: boolean;
  readonly trapMprotectExec?: boolean;
  readonly allowedJitBinaries?: readonly AuthorizedJitEngine[];
}


export interface PackageGuardConfig {
  readonly denyLifecycleScripts?: boolean;
  readonly managers?: readonly string[];
  readonly shannonEntropyThresholdBits?: number;
  readonly bypassRunners?: readonly string[];
}


export interface SupplyChainConfig extends PluginEnablement {
  readonly wxJit?: WxJitConfig;
  readonly packageGuard?: PackageGuardConfig;
}


export function supplyChain<P = unknown>(config: SupplyChainConfig): KelirBuilder<"supply-chain", unknown, never, P> {
  return createPlugin<"supply-chain", unknown, SupplyChainConfig, never, P>("supply-chain", { enabled: config.enabled, config });
}


export interface ToolGuardConfig extends PluginEnablement {
  readonly maxInvocationsPerTrace?: number;
  readonly pinSchemas?: boolean;
}


export function toolGuard<P = unknown>(config: ToolGuardConfig): KelirBuilder<"tool-guard", unknown, never, P> {
  return createPlugin<"tool-guard", unknown, ToolGuardConfig, never, P>("tool-guard", { enabled: config.enabled, config });
}


export interface TransactionConfig extends PluginEnablement {
  readonly timeoutMs?: number;
  readonly autoRollbackOnTaint?: boolean;
  readonly flushFuseBeforeCommit?: boolean;
}


export function transaction<P = unknown>(config: TransactionConfig): KelirBuilder<"transaction", unknown, never, P> {
  return createPlugin<"transaction", unknown, TransactionConfig, never, P>("transaction", { enabled: config.enabled, config });
}


export interface VaultConfig extends PluginEnablement {
  readonly metadataVsockPort?: number;
  readonly tokenTtlSeconds?: number;
}


export function vault<P = unknown>(config: VaultConfig): KelirBuilder<"vault", unknown, never, P> {
  return createPlugin<"vault", unknown, VaultConfig, never, P>("vault", { enabled: config.enabled, config });
}


export interface ListenConfig {
  readonly port?: number;
  readonly protocol?: string;
}


export interface ParkingConfig {
  readonly maxBufferedBytes?: number;
  readonly idleTimeoutMs?: number;
}


export interface WakeOnPacketConfig {
  readonly transportChannel?: number;
}


export interface BacklogConfig {
  readonly maxConnections?: number;
  readonly overflow?: "reject-503";
}


export interface WireIngressConfig extends PluginEnablement {
  readonly listen?: ListenConfig;
  readonly parking?: ParkingConfig;
  readonly wakeOnPacket?: WakeOnPacketConfig;
  readonly backlog?: BacklogConfig;
}


export function wireIngress<P = unknown>(config: WireIngressConfig): KelirBuilder<"wire-ingress", unknown, never, P> {
  return createPlugin<"wire-ingress", unknown, WireIngressConfig, never, P>("wire-ingress", { enabled: config.enabled, config });
}
