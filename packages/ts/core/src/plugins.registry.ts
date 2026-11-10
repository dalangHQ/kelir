// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { ChaosHandle, LedgerHandle, MeshHandle } from "./context.lifecycle.types.js";
import type { KelirPlugin, PluginContext } from "./plugin.model.js";

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
  readonly simulateError?: string;
  readonly errorProbability?: number;
}


export interface ChaosConfig extends PluginEnablement {
  readonly network?: readonly NetworkFaultRule[];
  readonly fs?: readonly StorageFaultRule[];
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function chaos(
  config: ChaosConfig,
): KelirPlugin & PluginContext<{ chaos: ChaosHandle }> {
  return { name: "chaos", enabled: config.enabled, config };
}


export interface ContainerConfig extends PluginEnablement {
  readonly runtime?: string;
  readonly isolateVarLibContainers?: boolean;
  readonly pinSubuidRanges?: boolean;
  readonly scratchDevice?: string;
}


export function container(config: ContainerConfig): KelirPlugin {
  return { name: "container", enabled: config.enabled, config };
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


export function dbProxy(config: DbProxyConfig): KelirPlugin {
  return { name: "db-proxy", enabled: config.enabled, config };
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
  readonly otel?: OtelConfig;
  readonly receipts?: ReceiptConfig;
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function ledger(
  config: LedgerConfig,
): KelirPlugin & PluginContext<{ ledger: LedgerHandle }> {
  return { name: "ledger", enabled: config.enabled, config };
}


export interface McpConfig extends PluginEnablement {
}


export function mcp(config: McpConfig): KelirPlugin {
  return { name: "mcp", enabled: config.enabled, config };
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
export function mesh(
  config: MeshConfig,
): KelirPlugin & PluginContext<{ mesh: MeshHandle }> {
  return { name: "mesh", enabled: config.enabled, config };
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


export function supplyChain(config: SupplyChainConfig): KelirPlugin {
  return { name: "supply-chain", enabled: config.enabled, config };
}


export interface ToolGuardConfig extends PluginEnablement {
  readonly maxInvocationsPerTrace?: number;
  readonly pinSchemas?: boolean;
}


export function toolGuard(config: ToolGuardConfig): KelirPlugin {
  return { name: "tool-guard", enabled: config.enabled, config };
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


export function wireIngress(config: WireIngressConfig): KelirPlugin {
  return { name: "wire-ingress", enabled: config.enabled, config };
}
