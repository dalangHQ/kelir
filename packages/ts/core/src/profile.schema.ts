// see: cans/plugins/profile.md#Session-compute-schema-(profile)
// see: cans/plugins/profile.md#Invariants-and-policy-(profile)
// see: cans/artifacts/examples/fixtures/kelir.yaml#session

import { Type, type Static } from "@sinclair/typebox";
import { Value } from "@sinclair/typebox/value";
import { Section, stringify as stringifyToml } from "@ltd/j-toml";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { EConfig } from "./errors.sdk.js";
import { readProfileText } from "./profile.schema.run.js";
import { parse as parseToml } from "@ltd/j-toml";
import type {
  KelirConfig,
  L4ProxyConfig,
  StorageConfig,
  TerminalConfig,
  WireConfig,
} from "./builder.config.js";

export type ProfileFormat = "toml" | "yaml";

const primitive = Type.Union([Type.Boolean(), Type.Integer(), Type.String()]);

const ProfileSchema = Type.Object(
  {
    version: Type.Optional(Type.Integer()),
    invariants: Type.Optional(Type.Unknown()),
    driver: Type.Optional(Type.Unknown()),
    session: Type.Optional(
      Type.Object({
        id: Type.Optional(Type.String()),
        tenant_id: Type.Optional(Type.String()),
        topology: Type.Optional(Type.String()),
        isolation_envelope: Type.Optional(
          Type.Object({
            enforce_openat2_beneath: Type.Optional(Type.Boolean()),
            cgroup_parent: Type.Optional(Type.String()),
          }),
        ),
        budget: Type.Optional(
          Type.Object({
            exec_seconds: Type.Optional(Type.Integer()),
          }),
        ),
        compute: Type.Optional(
          Type.Object({
            vcpus: Type.Optional(Type.Integer()),
            max_vcpus: Type.Optional(Type.Integer()),
            memory_mb: Type.Optional(Type.Integer()),
            page_size_kb: Type.Optional(Type.Integer()),
          }),
        ),
        balloon: Type.Optional(
          Type.Object({
            enabled: Type.Optional(Type.Boolean()),
            initial_mb: Type.Optional(Type.Integer()),
            pressure_reclaim: Type.Optional(Type.Boolean()),
            reclaim_strategy: Type.Optional(Type.String()),
          }),
        ),
        clock: Type.Optional(
          Type.Object({
            tsc_locked: Type.Optional(Type.Boolean()),
            pinned_clocksource: Type.Optional(Type.String()),
            deterministic_clock: Type.Optional(Type.Boolean()),
            preemption_grace_ms: Type.Optional(Type.Integer()),
          }),
        ),
        watchdog: Type.Optional(
          Type.Object({
            enabled: Type.Optional(Type.Boolean()),
            timeout_seconds: Type.Optional(Type.Integer()),
            action: Type.Optional(Type.String()),
            heartbeat_cadence_seconds: Type.Optional(Type.Integer()),
          }),
        ),
        devices: Type.Optional(
          Type.Object({
            virtio_rng: Type.Optional(
              Type.Object({
                enabled: Type.Optional(Type.Boolean()),
                source: Type.Optional(Type.String()),
              }),
            ),
            virtio_shm: Type.Optional(
              Type.Object({
                enabled: Type.Optional(Type.Boolean()),
                regions: Type.Optional(
                  Type.Array(Type.Record(Type.String(), primitive)),
                ),
              }),
            ),
          }),
        ),
        artifacts: Type.Optional(
          Type.Object({
            kernel_path: Type.Optional(Type.String()),
            rootfs_erofs_path: Type.Optional(Type.String()),
            kernel_boot_params: Type.Optional(Type.Array(Type.String())),
          }),
        ),
        shim: Type.Optional(
          Type.Object({
            max_exec_slots: Type.Optional(Type.Integer()),
            hide_pid: Type.Optional(Type.Integer()),
            anti_toctou_cgroups: Type.Optional(Type.Boolean()),
            granular_task_freeze: Type.Optional(Type.Boolean()),
            task_freeze_timeout_ms: Type.Optional(Type.Integer()),
            rejection_mode: Type.Optional(Type.String()),
            mask_proc_environ: Type.Optional(Type.Boolean()),
          }),
        ),
        driver: Type.Optional(
          Type.Object({
            fast_boot: Type.Optional(Type.String()),
          }),
        ),
        storage: Type.Optional(
          Type.Object({
            dax_window: Type.Optional(Type.String()),
          }),
        ),
      }),
    ),
    storage: Type.Optional(Type.Unknown()),
    wire: Type.Optional(Type.Unknown()),
    terminal: Type.Optional(Type.Unknown()),
    checkpoint: Type.Optional(Type.Unknown()),
    host: Type.Optional(Type.Unknown()),
    plugins: Type.Optional(Type.Unknown()),
    attestation: Type.Optional(Type.Unknown()),
  },
  { additionalProperties: false },
);

export type ProfileSpec = Static<typeof ProfileSchema>;

// L3-GAP: [bindings_ts]: route non-session namespaces to their ConfigurablePlugin schema owners per cans/plugins/profile.md#Inversion-of-Control-configuration-router.; slop: non-session namespace silently dropped from config
export function parseProfile(
  text: string,
  format: ProfileFormat,
): ProfileSpec {
  const loaded: unknown =
    format === "toml" ? parseToml(text, { bigint: false }) : parseYaml(text);
  if (!Value.Check(ProfileSchema, loaded)) {
    let detail = "profile root must be a table";
    for (const error of Value.Errors(ProfileSchema, loaded)) {
      detail = `profile schema violation at ${error.path === "" ? "/" : error.path}: ${error.message}`;
      break;
    }
    throw new EConfig(`parseProfile: ${detail}`);
  }
  return Value.Clean(ProfileSchema, loaded) as ProfileSpec;
}

export function profileFormatForPath(path: string): ProfileFormat {
  return path.endsWith(".yaml") || path.endsWith(".yml") ? "yaml" : "toml";
}

// see: cans/plugins/profile.md#Declarative-profile-execution
export async function readProfile(path: string): Promise<ProfileSpec> {
  if (
    !path.endsWith(".toml") &&
    !path.endsWith(".yaml") &&
    !path.endsWith(".yml")
  ) {
    throw new EConfig(
      `readProfile: unsupported profile extension for \`${path}\` (expected .toml, .yaml, or .yml)`,
    );
  }
  const text = await readProfileText(path);
  return parseProfile(text, profileFormatForPath(path));
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function asBool(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function asInt(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) ? value : undefined;
}

function asStr(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

// see: cans/plugins/profile.md#Session-compute-schema-(profile)
export function configFromProfile(profile: ProfileSpec): KelirConfig {
  const { session } = profile;
  if (session === undefined) {
    return {};
  }
  return {
    vcpus: session.compute?.vcpus,
    memoryMb: session.compute?.memory_mb,
    pageSizeKb: session.compute?.page_size_kb,
    tscLocked: session.clock?.tsc_locked,
    pinnedClocksource: session.clock?.pinned_clocksource,
    deterministicClock: session.clock?.deterministic_clock,
    preemptionGraceMs: session.clock?.preemption_grace_ms,
    enableWatchdog: session.watchdog?.enabled,
    kernel: session.artifacts?.kernel_path,
    rootfsErofs: session.artifacts?.rootfs_erofs_path,
  };
}

// see: cans/04-terminal-plane.md#Terminal-pipeline
export function terminalConfigFromProfile(
  profile: ProfileSpec,
): TerminalConfig | undefined {
  const terminal = asRecord(profile.terminal);
  if (terminal === undefined) {
    return undefined;
  }
  const pty = asRecord(terminal.pty);
  const sanitizer = asRecord(terminal.sanitizer);
  const wireframeGrid = asRecord(terminal.wireframe_grid);
  return {
    cols: asInt(pty?.cols),
    rows: asInt(pty?.rows),
    wireframeGrid:
      wireframeGrid === undefined
        ? undefined
        : {
            trackScreenMatrix: asBool(wireframeGrid.track_screen_matrix),
            exportOnTrap: asBool(wireframeGrid.export_on_trap),
          },
    sanitizer:
      sanitizer === undefined
        ? undefined
        : {
            dropOsc52: asBool(sanitizer.drop_osc52),
            stripTitle: asBool(sanitizer.strip_title),
          },
  };
}

// see: cans/05-storage-plane.md#Storage-architecture
export function storageConfigFromProfile(
  profile: ProfileSpec,
): StorageConfig | undefined {
  const storage = asRecord(profile.storage);
  if (storage === undefined) {
    return undefined;
  }
  const idmap = asRecord(storage.idmap);
  const sharedCaches = Array.isArray(storage.shared_caches)
    ? storage.shared_caches.flatMap((entry) => {
        const cache = asRecord(entry);
        if (cache === undefined) {
          return [];
        }
        return [
          {
            hostPath: asStr(cache.host_path) ?? "",
            guestMount: asStr(cache.guest_mount) ?? "",
            readOnly: asBool(cache.read_only),
          },
        ];
      })
    : undefined;
  return {
    idmap:
      idmap === undefined
        ? undefined
        : {
            guestUid: asInt(idmap.guest_uid) ?? 0,
            hostUid: asInt(idmap.host_uid) ?? 0,
          },
    sharedCaches,
    encapsulation: asStr(storage.encapsulation),
    daxWindow: asStr(storage.dax_window),
  };
}

// see: cans/07-wire-plane.md#Transparent-L4-stream-diversion
function l4ProxyFromEntry(entry: unknown): L4ProxyConfig | undefined {
  const record = asRecord(entry);
  const listenPort = asInt(record?.listen_port);
  if (record === undefined || listenPort === undefined) {
    return undefined;
  }
  const mtls = asRecord(record.inject_mtls);
  return {
    listenPort,
    destination: asStr(record.destination),
    forwardUnixSocket: asStr(record.forward_unix_socket),
    injectMtls:
      mtls === undefined
        ? undefined
        : {
            certPath: asStr(mtls.cert_path) ?? "",
            keyPath: asStr(mtls.key_path) ?? "",
          },
  };
}

// see: cans/07-wire-plane.md#DNS-policy-filter
export function wireConfigFromProfile(
  profile: ProfileSpec,
): WireConfig | undefined {
  const wire = asRecord(profile.wire);
  if (wire === undefined) {
    return undefined;
  }
  const proxies = asRecord(wire.l4_diversion)?.proxies;
  return {
    l4Proxies: Array.isArray(proxies)
      ? proxies.flatMap((entry): L4ProxyConfig[] => {
          const proxy = l4ProxyFromEntry(entry);
          return proxy === undefined ? [] : [proxy];
        })
      : undefined,
  };
}

// see: cans/15-operations.md#Multi-language-parity-gate
export interface HostPlaneConfig {
  readonly reaper?: Record<string, unknown>;
  readonly cgroups?: Record<string, unknown>;
  readonly catalog?: Record<string, unknown>;
  readonly telemetryRing?: Record<string, unknown>;
}

// see: cans/15-operations.md#Multi-language-parity-gate
export interface CheckpointPlaneConfig {
  readonly stepDag?: Record<string, unknown>;
  readonly compaction?: Record<string, unknown>;
  readonly snapshotBoundaries?: Record<string, unknown>;
}

// see: cans/plugins/profile.md#Host-schema-(profile)
export function hostConfigFromProfile(
  profile: ProfileSpec,
): HostPlaneConfig | undefined {
  const host = asRecord(profile.host);
  if (host === undefined) {
    return undefined;
  }
  return {
    reaper: asRecord(host.reaper),
    cgroups: asRecord(host.cgroups),
    catalog: asRecord(host.catalog),
    telemetryRing: asRecord(host.telemetry_ring),
  };
}

// see: cans/plugins/profile.md#Checkpoint-schema-(profile)
export function checkpointConfigFromProfile(
  profile: ProfileSpec,
): CheckpointPlaneConfig | undefined {
  const checkpoint = asRecord(profile.checkpoint);
  if (checkpoint === undefined) {
    return undefined;
  }
  return {
    stepDag: asRecord(checkpoint.step_dag),
    compaction: asRecord(checkpoint.compaction),
    snapshotBoundaries: asRecord(checkpoint.boundaries),
  };
}

type TomlValue = string | number | boolean | TomlTable | readonly TomlValue[];
type TomlTable = { [key: string]: TomlValue };

function toTomlTables(value: TomlValue): TomlValue {
  if (Array.isArray(value)) {
    return value.map(toTomlTables);
  }
  if (typeof value === "object" && value !== null) {
    const table: TomlTable = {};
    for (const [key, entry] of Object.entries(value)) {
      table[key] = toTomlTables(entry);
    }
    return Section(table);
  }
  return value;
}

// L3-GAP: [bindings_ts]: full-fidelity round-trip of arbitrary profile sections once plugin namespace schemas register.; slop: profile sections lost in round-trip, namespace silently dropped
export function serializeProfile(
  profile: ProfileSpec,
  format: ProfileFormat,
): string {
  if (format === "yaml") {
    return stringifyYaml(profile);
  }
  return stringifyToml(toTomlTables(profile as TomlTable) as TomlTable, {
    integer: Number.MAX_SAFE_INTEGER,
    newline: "\n",
    newlineAround: "section",
  });
}
