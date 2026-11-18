// see: cans/plugins/profile.md#Plugin-profile
// see: cans/plugins/profile.md#Declarative-profile-execution
// see: cans/02-plugin-model.md#Unified-builder

import { KelirBuilder } from "@kelir/core";

async function main() {
  const builder = await KelirBuilder.fromConfig("./fixtures/kelir.toml");
  builder.seal();
  const session = await builder.boot();

  const r = await session.spawn("git", ["status"]);
  console.error(`[kelir] git status -> exit=${r.exitCode}`);

  await session.exportSpec("./fixtures/kelir.exported.toml");

  await session.terminate();
}

main().catch(console.error);
