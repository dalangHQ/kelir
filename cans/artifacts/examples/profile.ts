// see: cans/plugins/profile.md#Plugin-profile
// see: cans/plugins/profile.md#Declarative-profile-execution

import { Sandbox } from "@kelir/core";

async function main() {
  const box = await Sandbox.fromConfig("./fixtures/kelir.toml");
  await box.boot();

  const r = await box.exec("git", ["status"]);
  console.error(`[kelir] git status -> exit=${r.exitCode}`);

  await box.exportSpec("./fixtures/kelir.exported.toml");

  await box.terminate();
}

main().catch(console.error);
