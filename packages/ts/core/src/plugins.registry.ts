// see: cans/03-sdk-architecture.md#TypeScript-SDK

import type { ChaosHandle, ContainerHandle, DbProxyHandle, LedgerHandle, MeshHandle, TransactionHandle } from "./context.lifecycle.types.js";
import { createPlugin } from "./builder.lifecycle.js";
import type { KelirBuilder } from "./builder.lifecycle.js";

export interface PluginEnablement {
  readonly enabled?: boolean;
}


export interface AnomalyConfig extends PluginEnablement {
  readonly slidingWindowDepth?: number;
  readonly exportFeatureVector?: boolean;
  readonly featureExtraction?: boolean;
  readonly surprisalThreshold?: number;
}


export function anomaly<P = unknown>(config: AnomalyConfig): KelirBuilder<"anomaly", unknown, never, P> {
  return createPlugin<"anomaly", unknown, AnomalyConfig, never, P>("anomaly", { enabled: config.enabled, config });
}


export interface NetworkFaultRule {
  readonly target?: string;
  readonly direction?: "outbound" | "inbound" | "both";
  readonly latencyMs?: number;
  readonly jitterMs?: number;
  readonly dropRate?: number;
  readonly injectRst?: boolean;
  readonly bandwidthKbps?: number;
}


export interface StorageFaultRule {
  readonly targetGlob?: string;
  readonly simulateErrno?: "eperm" | "eacces" | "enoent" | "einval" | "enospc" | "erofs" | "econnrefused" | "enxio" | "ewouldblock" | "efault" | "ebusy" | "exdev" | "epipe" | "enodata" | "eproto" | "epolicy" | "etoolquota" | "ewxviolation" | "econfig" | "esecurityviolation" | "eunknownkind" | "eclockregress" | "path-traversal" | "etimeout" | "eafnosupport";
  readonly probability?: number;
}


export interface ChaosConfig extends PluginEnablement {
  readonly network?: readonly NetworkFaultRule[];
  readonly storage?: readonly StorageFaultRule[];
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


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function container<P = unknown>(config: ContainerConfig): KelirBuilder<"container", { container: ContainerHandle }, never, P> {
  return createPlugin<"container", { container: ContainerHandle }, ContainerConfig, never, P>("container", { enabled: config.enabled, config });
}


export interface PgSchemaRewrite {
  readonly fromSchema?: string;
  readonly toSchema?: string;
}


export interface PgWireRewrite {
  readonly listenPort?: number;
  readonly upstream?: string;
  readonly schemaRewrites?: readonly PgSchemaRewrite[];
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


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function dbProxy<P = unknown>(config: DbProxyConfig): KelirBuilder<"db-proxy", { "db-proxy": DbProxyHandle }, never, P> {
  return createPlugin<"db-proxy", { "db-proxy": DbProxyHandle }, DbProxyConfig, never, P>("db-proxy", { enabled: config.enabled, config });
}


export interface EgressProxyConfig extends PluginEnablement {
  readonly httpProxy?: string;
  readonly httpsProxy?: string;
  readonly allProxy?: string;
  readonly noProxy?: readonly string[];
}


export function egressProxy<P = unknown>(config: EgressProxyConfig): KelirBuilder<"egress-proxy", unknown, never, P> {
  return createPlugin<"egress-proxy", unknown, EgressProxyConfig, never, P>("egress-proxy", { enabled: config.enabled, config });
}


export interface SinkholeRule {
  readonly host?: string;
  readonly methods?: readonly string[];
  readonly paths?: readonly string[];
  readonly allow?: boolean;
}


export interface SinkholeConfig {
  readonly rules?: readonly SinkholeRule[];
}


export interface L7HttpConfig extends PluginEnablement {
  readonly sinkhole?: SinkholeConfig;
  readonly protocolInspection?: boolean;
}


export function l7Http<P = unknown>(config: L7HttpConfig): KelirBuilder<"l7-http", unknown, never, P> {
  return createPlugin<"l7-http", unknown, L7HttpConfig, never, P>("l7-http", { enabled: config.enabled, config });
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
  readonly storePath?: string;
  readonly merkleChain?: boolean;
  readonly suppressLocalDb?: boolean;
  readonly hashingStandard?: "rfc-6962-dual-blake-3-sha-256";
  readonly recordPreRedactionHash?: boolean;
  readonly includeHookSpansInRoot?: boolean;
  readonly witnessCosigningEndpoint?: string;
  readonly otel?: OtelConfig;
  readonly receipts?: ReceiptConfig;
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function ledger<P = unknown>(config: LedgerConfig): KelirBuilder<"ledger", { ledger: LedgerHandle }, never, P> {
  return createPlugin<"ledger", { ledger: LedgerHandle }, LedgerConfig, never, P>("ledger", { enabled: config.enabled, config });
}


export interface McpConfig extends PluginEnablement {
  readonly maxFrameBytes?: number;
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


export interface MaskReadRule {
  readonly glob?: string;
  readonly patterns?: readonly ([string, string])[];
}


export interface MaskReadConfig {
  readonly engine?: "fast-regex" | "streaming-automata";
  readonly carryoverBytes?: number;
  readonly rules?: readonly MaskReadRule[];
}


export interface RedactionConfig extends PluginEnablement {
  readonly maskRead?: MaskReadConfig;
}


export function redaction<P = unknown>(config: RedactionConfig): KelirBuilder<"redaction", unknown, never, P> {
  return createPlugin<"redaction", unknown, RedactionConfig, never, P>("redaction", { enabled: config.enabled, config });
}


export interface ReplayConfig extends PluginEnablement {
  readonly stepWindowInstructions?: number;
  readonly pinnedClocksource?: string;
  readonly deterministicRngSeed?: number;
  readonly networkIsolation?: boolean;
}


export function replay<P = unknown>(config: ReplayConfig): KelirBuilder<"replay", unknown, never, P> {
  return createPlugin<"replay", unknown, ReplayConfig, never, P>("replay", { enabled: config.enabled, config });
}


export interface StepDagConfig {
  readonly enabled?: boolean;
  readonly requireSignedAttribution?: boolean;
  readonly requireSeparationOfDuties?: boolean;
  readonly storage?: "content-addressed";
}


export interface StepEvalAssertions {
  readonly assertBinaryBlake3?: boolean;
  readonly maxEgressBytesHermetic?: number;
  readonly requireNonZeroDelta?: boolean;
  readonly maxDurationSeconds?: number;
}


export interface StepEvaluationConfig {
  readonly enabled?: boolean;
  readonly defaultAction?: "proceed" | "rollback-and-halt";
  readonly assertions?: StepEvalAssertions;
}


export interface StepEvalConfig extends PluginEnablement {
  readonly sentinelPath?: string;
  readonly ptyQuiescence?: boolean;
  readonly stepDag?: StepDagConfig;
  readonly stepEvaluation?: StepEvaluationConfig;
}


export function stepEval<P = unknown>(config: StepEvalConfig): KelirBuilder<"step-eval", unknown, never, P> {
  return createPlugin<"step-eval", unknown, StepEvalConfig, never, P>("step-eval", { enabled: config.enabled, config });
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


export interface TaintConfig extends PluginEnablement {
}


export function taint<P = unknown>(config: TaintConfig): KelirBuilder<"taint", unknown, never, P> {
  return createPlugin<"taint", unknown, TaintConfig, never, P>("taint", { enabled: config.enabled, config });
}


export interface PromptRuleConfig {
  readonly pattern?: string;
  readonly timeoutSeconds?: number;
  readonly verifyForeground?: boolean;
  readonly action?: readonly number[];
}


export interface TerminalGuardConfig extends PluginEnablement {
  readonly prompts?: readonly PromptRuleConfig[];
  readonly noticeTemplate?: string;
}


export function terminalGuard<P = unknown>(config: TerminalGuardConfig): KelirBuilder<"terminal-guard", unknown, never, P> {
  return createPlugin<"terminal-guard", unknown, TerminalGuardConfig, never, P>("terminal-guard", { enabled: config.enabled, config });
}


export interface TokenShieldConfig extends PluginEnablement {
  readonly maxBytesTotal?: number;
  readonly maxLinesTotal?: number;
  readonly clampStrategy?: "head-and-tail" | "head-only" | "tail-only";
  readonly notice?: string;
}


export function tokenShield<P = unknown>(config: TokenShieldConfig): KelirBuilder<"token-shield", unknown, never, P> {
  return createPlugin<"token-shield", unknown, TokenShieldConfig, never, P>("token-shield", { enabled: config.enabled, config });
}


export interface ToolGuardConfig extends PluginEnablement {
  readonly maxInvocationsPerTrace?: number;
  readonly pinSchemas?: boolean;
  readonly maxFrameBytes?: number;
  readonly detectSemanticRugPull?: boolean;
  readonly strictAstParameterValidation?: boolean;
  readonly propagateByteRangeTaints?: boolean;
}


export function toolGuard<P = unknown>(config: ToolGuardConfig): KelirBuilder<"tool-guard", unknown, never, P> {
  return createPlugin<"tool-guard", unknown, ToolGuardConfig, never, P>("tool-guard", { enabled: config.enabled, config });
}


export interface TransactionConfig extends PluginEnablement {
  readonly timeoutMs?: number;
  readonly autoRollbackOnTaint?: boolean;
  readonly flushFuseBeforeCommit?: boolean;
}


// see: cans/03-sdk-architecture.md#Service-lookup-across-languages
export function transaction<P = unknown>(config: TransactionConfig): KelirBuilder<"transaction", { transaction: TransactionHandle }, never, P> {
  return createPlugin<"transaction", { transaction: TransactionHandle }, TransactionConfig, never, P>("transaction", { enabled: config.enabled, config });
}


export interface CredentialDescriptor {
  readonly name?: string;
  readonly kind?: "bearer-token" | "sig-v-4" | "mtls-bundle";
  readonly audience?: string;
  readonly placeholder?: string;
}


export interface VaultConfig extends PluginEnablement {
  readonly metadataVsockPort?: number;
  readonly tokenTtlSeconds?: number;
  readonly credentials?: readonly CredentialDescriptor[];
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
