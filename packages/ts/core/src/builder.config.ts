// see: cans/03-sdk-architecture.md#TypeScript-SDK

// see: cans/03-sdk-architecture.md#TypeScript-SDK
// see: cans/01-architecture.md#Embedded-profile
export interface KelirConfig {
  readonly vcpus?: number;
  readonly memoryMb?: number;
  readonly pageSizeKb?: number;
  readonly deterministicClock?: boolean;
  readonly pinnedClocksource?: string;
  readonly preemptionGraceMs?: number;
  readonly driver?: string;
  readonly kernel?: string;
  readonly rootfsErofs?: string;
  readonly tscLocked?: boolean;
  readonly enableWatchdog?: boolean;
  readonly detached?: boolean;
  readonly networkOff?: boolean;
  readonly nsPerInstruction?: number;
  readonly entropySeed?: number;
  readonly ffiBoundary?: FfiBoundaryProfile;
}

// see: cans/01-architecture.md#Embedded-profile
export interface FfiBoundaryProfile {
  readonly embedMode: "in_process_libkelir";
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalStream {
  write(data: string): void;
}

// see: cans/04-terminal-plane.md#Headless-VT100-wireframe-grid
export interface TerminalWireframeGridConfig {
  readonly trackScreenMatrix?: boolean;
  readonly exportOnTrap?: boolean;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalSanitizerConfig {
  readonly dropOsc52?: boolean;
  readonly stripTitle?: boolean;
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export interface TerminalConfig {
  readonly cols?: number;
  readonly rows?: number;
  readonly wireframeGrid?: TerminalWireframeGridConfig;
  readonly sanitizer?: TerminalSanitizerConfig;
  readonly streamEngine?: string;
}

// see: cans/05-storage-plane.md#Workspace-commit
export interface StorageConfig {
  readonly idmap?: { readonly guestUid: number; readonly hostUid: number };
  readonly sharedCaches?: readonly {
    readonly hostPath: string;
    readonly guestMount: string;
    readonly readOnly?: boolean;
  }[];
  readonly lockBroker?: boolean;
  readonly encapsulation?: string;
  readonly daxWindow?: string;
}

// see: cans/07-wire-plane.md#Sinkhole-rules
export interface SinkholeRule {
  readonly host: string;
  readonly allow?: boolean;
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export interface L4ProxyConfig {
  readonly listenPort: number;
  readonly destination?: string;
  readonly forwardUnixSocket?: string;
  readonly injectMtls?: {
    readonly certPath: string;
    readonly keyPath: string;
  };
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export interface WireConfig {
  readonly sinkhole?: {
    readonly rules?: readonly SinkholeRule[];
  };
  readonly sshAgentBridge?: boolean;
  readonly dialTimeoutMs?: number;
  readonly l4Proxies?: readonly L4ProxyConfig[];
  readonly tapFilter?: string;
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
export interface WireScope {
  divertPortToUnix(listenPort: number, unixSocket: string): WireScope;
}

// see: cans/05-storage-plane.md#Host-inode-lock-broker
export interface LockBrokerConfig {
  readonly globs: readonly string[];
  readonly nonBlocking?: boolean;
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
export interface CreatePluginOptions<Opts = unknown> {
  readonly opts?: Opts;
  readonly enabled?: boolean;
  readonly config?: Opts;
}
