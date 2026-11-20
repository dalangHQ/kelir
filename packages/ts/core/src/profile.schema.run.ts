// see: cans/plugins/profile.md#Declarative-profile-execution

import { EConfig } from "./errors.sdk.js";

// TODO (L3) [bindings_ts]: read the profile file from disk once the file bridge lands; slop: profile read never lands, from_config throws instead of loading.
export async function readProfileText(path: string): Promise<string> {
  void path;
  throw new EConfig("readProfile: profile file read is not implemented (L3)");
}

// TODO (L3) [bindings_ts]: write the serialized profile to disk once the file bridge lands; slop: export_spec never writes, the serialized profile is dropped.
export async function writeProfileText(
  path: string,
  text: string,
): Promise<void> {
  void path;
  void text;
  throw new EConfig(
    "writeProfile: profile file write is not implemented (L3)",
  );
}
