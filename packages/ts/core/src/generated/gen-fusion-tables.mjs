// see: cans/02-plugin-model.md#Segregated-ring-traits
// see: cans/02-plugin-model.md#Hook-context-restriction
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const TS_ENUM_NAMES = { TrampolineRing: "RingEvent", HookStage: "HookStage" };
const PY_ENUM_NAMES = { TrampolineRing: "RingEvent", HookStage: "HookStage" };
const RING_ENUM = "TrampolineRing";
const STAGE_ENUM = "HookStage";

function fail(message) {
  throw new Error(`gen-fusion-tables: ${message}`);
}

function requireRepoRoot() {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const root = resolve(scriptDir, "../../../../../");
  if (!statSync(join(root, "crates/kelir-interface/src/fusion.rs"), { throwIfNoEntry: false })) {
    fail(`cannot locate crates/kelir-interface/src/fusion.rs below ${root}`);
  }
  return root;
}

function extractTagEnumInvocations(text) {
  const invocations = [];
  const marker = "tag_enum!";
  let at = text.indexOf(marker);
  while (at !== -1) {
    const open = text.indexOf("{", at + marker.length);
    if (open === -1) {
      fail("tag_enum! invocation has no opening brace");
    }
    let depth = 0;
    let close = -1;
    for (let i = open; i < text.length; i += 1) {
      if (text[i] === "{") depth += 1;
      else if (text[i] === "}") {
        depth -= 1;
        if (depth === 0) {
          close = i;
          break;
        }
      }
    }
    if (close === -1) {
      fail("tag_enum! invocation has no closing brace");
    }
    invocations.push(text.slice(open + 1, close));
    at = text.indexOf(marker, close + 1);
  }
  return invocations;
}

function parseTagEnum(body) {
  const header = /pub\s+enum\s+(\w+)\s*\{([^}]*)\}/.exec(body);
  if (header === null) {
    fail("tag_enum! body declares no `pub enum`");
  }
  const variants = [];
  const seenVariants = new Set();
  const seenTags = new Set();
  for (const match of body.matchAll(/(\w+)\s*=>\s*"([^"]+)"/g)) {
    const [, variant, tag] = match;
    if (seenVariants.has(variant)) fail(`duplicate variant ${variant}`);
    if (seenTags.has(tag)) fail(`duplicate tag ${tag}`);
    seenVariants.add(variant);
    seenTags.add(tag);
    variants.push({ variant, tag });
  }
  if (variants.length === 0) fail(`enum ${header[1]} declares no variants`);
  return { name: header[1], variants };
}

function parseStringArray(text, name) {
  const match = new RegExp(`pub\\s+const\\s+${name}[^=]*=\\s*&?\\s*\\[([^\\]]*)\\]`).exec(text);
  if (match === null) fail(`const ${name} not found`);
  return [...match[1].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]);
}

function parseHookRings(text) {
  const match = /pub\s+const\s+HOOK_RINGS[^=]*=\s*&?\s*\[([\s\S]*?)\];/.exec(text);
  if (match === null) fail("const HOOK_RINGS not found");
  const bindings = [];
  const seenMethods = new Set();
  for (const entry of match[1].matchAll(
    /method:\s*"([^"]+)",\s*ring:\s*TrampolineRing::(\w+),\s*lifecycle_tag:\s*(None|Some\("([^"]+)"\))/g,
  )) {
    const [, method, ring, lifecycle, tag] = entry;
    if (seenMethods.has(method)) fail(`duplicate hook method ${method}`);
    seenMethods.add(method);
    bindings.push({ method, ring, lifecycleTag: lifecycle === "None" ? null : tag });
  }
  if (bindings.length === 0) fail("HOOK_RINGS declares no bindings");
  return bindings;
}

function parseSource(text) {
  const enums = extractTagEnumInvocations(text).map(parseTagEnum);
  if (enums.length !== Object.keys(TS_ENUM_NAMES).length) {
    fail(`expected ${Object.keys(TS_ENUM_NAMES).length} tag_enum definitions, found ${enums.length}`);
  }
  for (const def of enums) {
    if (!(def.name in TS_ENUM_NAMES)) fail(`unknown tag_enum ${def.name}; teach the generator`);
  }
  const lifecycleTags = parseStringArray(text, "LIFECYCLE_TAGS");
  if (lifecycleTags.length === 0) fail("LIFECYCLE_TAGS is empty");
  if (new Set(lifecycleTags).size !== lifecycleTags.length) fail("duplicate LIFECYCLE_TAGS entry");
  const rings = enums.find((def) => def.name === RING_ENUM);
  const stages = enums.find((def) => def.name === STAGE_ENUM);
  const hookRings = parseHookRings(text);
  const ringVariants = new Set(rings.variants.map((entry) => entry.variant));
  for (const binding of hookRings) {
    if (!ringVariants.has(binding.ring)) fail(`HOOK_RINGS references unknown ring ${binding.ring}`);
    if (binding.lifecycleTag !== null) {
      if (binding.ring !== "Lifecycle") fail(`HOOK_RINGS pins lifecycle tag on ring ${binding.ring}`);
      if (!lifecycleTags.includes(binding.lifecycleTag)) {
        fail(`HOOK_RINGS references unknown lifecycle tag ${binding.lifecycleTag}`);
      }
    }
  }
  return { rings, stages, lifecycleTags, hookRings };
}

function tsName(rustName) {
  const projected = TS_ENUM_NAMES[rustName];
  if (projected === undefined) fail(`no TypeScript projection for ${rustName}`);
  return projected;
}

function pyMember(variant) {
  return variant.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();
}

function tsMethod(method) {
  return method.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
}

function emitTypescript(model) {
  const ringName = tsName(model.rings.name);
  const lines = [];
  lines.push("// see: cans/02-plugin-model.md#Segregated-ring-traits");
  lines.push("// see: cans/02-plugin-model.md#Hook-context-restriction");
  lines.push("// see: cans/02-plugin-model.md#Foreign-plugin-proxy");
  lines.push("");
  lines.push(`export enum ${ringName} {`);
  for (const { variant, tag } of model.rings.variants) {
    lines.push(`  ${variant} = "${tag}",`);
  }
  lines.push("}");
  lines.push("");
  const stageName = tsName(model.stages.name);
  const stageUnion = model.stages.variants.map((entry) => `"${entry.tag}"`).join(" | ");
  if (`export type ${stageName} = ${stageUnion};`.length <= 80) {
    lines.push(`export type ${stageName} = ${stageUnion};`);
  } else {
    lines.push(`export type ${stageName} =`);
    for (const { tag } of model.stages.variants) {
      lines.push(`  | "${tag}"`);
    }
    lines[lines.length - 1] += ";";
  }
  lines.push("");
  lines.push("export type LifecycleTag =");
  for (const tag of model.lifecycleTags) {
    lines.push(`  | "${tag}"`);
  }
  lines[lines.length - 1] += ";";
  lines.push("");
  lines.push("export const LIFECYCLE_TAGS: readonly string[] = [");
  for (const tag of model.lifecycleTags) {
    lines.push(`  "${tag}",`);
  }
  lines.push("];");
  lines.push("");
  lines.push("export interface HookRingBinding {");
  lines.push("  readonly method: string;");
  lines.push(`  readonly ring: ${ringName};`);
  lines.push("  readonly lifecycleTag?: LifecycleTag;");
  lines.push("}");
  lines.push("");
  lines.push("export const HOOK_RINGS: readonly HookRingBinding[] = [");
  for (const binding of model.hookRings) {
    const extra = binding.lifecycleTag === null ? "" : `, lifecycleTag: "${binding.lifecycleTag}"`;
    lines.push(`  { method: "${tsMethod(binding.method)}", ring: ${ringName}.${binding.ring}${extra} },`);
  }
  lines.push("];");
  return `${lines.join("\n")}\n`;
}

function emitPython(model) {
  const lines = [];
  lines.push("# see: cans/02-plugin-model.md#Segregated-ring-traits");
  lines.push("# see: cans/02-plugin-model.md#Hook-context-restriction");
  lines.push("# see: cans/02-plugin-model.md#Foreign-plugin-proxy");
  lines.push("from __future__ import annotations");
  lines.push("");
  lines.push("from enum import Enum");
  lines.push("from typing import Literal");
  lines.push("");
  lines.push("");
  lines.push(`class ${PY_ENUM_NAMES[model.rings.name]}(str, Enum):`);
  for (const { variant, tag } of model.rings.variants) {
    lines.push(`    ${pyMember(variant)} = "${tag}"`);
  }
  lines.push("");
  lines.push("");
  lines.push(`class ${PY_ENUM_NAMES[model.stages.name]}(str, Enum):`);
  for (const { variant, tag } of model.stages.variants) {
    lines.push(`    ${pyMember(variant)} = "${tag}"`);
  }
  lines.push("");
  lines.push("");
  lines.push("LifecycleTag = Literal[");
  for (const tag of model.lifecycleTags) {
    lines.push(`    "${tag}",`);
  }
  lines.push("]");
  lines.push("");
  lines.push("LIFECYCLE_TAGS: tuple[str, ...] = (");
  for (const tag of model.lifecycleTags) {
    lines.push(`    "${tag}",`);
  }
  lines.push(")");
  lines.push("");
  lines.push("HOOK_RINGS: dict[str, tuple[RingEvent, LifecycleTag | None]] = {");
  for (const binding of model.hookRings) {
    const tag = binding.lifecycleTag === null ? "None" : `"${binding.lifecycleTag}"`;
    lines.push(`    "${binding.method}": (RingEvent.${pyMember(binding.ring)}, ${tag}),`);
  }
  lines.push("}");
  return `${lines.join("\n")}\n`;
}

const repoRoot = requireRepoRoot();
const source = readFileSync(join(repoRoot, "crates/kelir-interface/src/fusion.rs"), "utf8");
const model = parseSource(source);
const outputs = [
  [join(repoRoot, "packages/ts/core/src/generated/fusion-tables.ts"), emitTypescript(model)],
  [join(repoRoot, "packages/py/kelir/generated/fusion_tables.py"), emitPython(model)],
];
for (const [target, content] of outputs) {
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, content, "utf8");
  process.stdout.write(`gen-fusion-tables: wrote ${target}\n`);
}
