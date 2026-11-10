// see: cans/plugins/profile.md#Session-compute-schema-(profile)
// see: cans/plugins/profile.md#Invariants-and-policy-(profile)
// see: cans/artifacts/examples/fixtures/kelir.yaml#session

import { Type, type Static } from "@sinclair/typebox";
import { Value } from "@sinclair/typebox/value";
import { Section, stringify as stringifyToml } from "@ltd/j-toml";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { EConfig } from "./errors.sdk.js";
import { parse as parseToml } from "@ltd/j-toml";

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
        topology: Type.Optional(Type.String()),
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

// TODO (L3): route non-session namespaces to their ConfigurablePlugin schema owners per cans/plugins/profile.md#Inversion-of-Control-configuration-router.
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

// TODO (L3): full-fidelity round-trip of arbitrary profile sections once plugin namespace schemas register.
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
