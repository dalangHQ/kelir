// see: cans/artifacts/test-plan/cases/bindings_ts/bt_02_typescript_vectors_parity_frame_verdict_equality.yaml#case_id
import test from "node:test";
import { readFile } from "node:fs/promises";

// see: cans/artifacts/test-plan/cases/bindings_ts/bt_02_typescript_vectors_parity_frame_verdict_equality.yaml#case_id
interface ParityVectorEntry {
  readonly name: string;
  readonly expected_verdict: unknown;
  readonly frame: { readonly frame_hex: string };
}

interface ParityVectorResult {
  readonly frame_hex: string;
  readonly verdict: unknown;
}

// see: cans/artifacts/test-plan/cases/bindings_ts/bt_02_typescript_vectors_parity_frame_verdict_equality.yaml#case_id
async function bt_02_typescript_vectors_parity_frame_verdict_equality(): Promise<void> {
  const vectorsUrl = new URL("../../../../../vectors.json", import.meta.url);
  const raw = JSON.parse(await readFile(vectorsUrl, "utf8")) as {
    vectors: readonly ParityVectorEntry[];
  };
  if (raw.vectors.length !== 12) {
    throw new Error(
      `bt_02: vectors.json carries ${raw.vectors.length} vectors, expected 12`,
    );
  }
  if (raw.vectors[0].name !== "trap-git-commit-pass") {
    throw new Error(
      `bt_02: vectors.json first vector is ${raw.vectors[0].name}, expected trap-git-commit-pass`,
    );
  }
  const mod = (await import("../../dist/index.js")) as Record<
    string,
    unknown
  >;
  const consumer = mod.runParityVectors;
  if (typeof consumer !== "function") {
    throw new Error(
      "bt_02: @kelir/core exports no parity vector consumer (runParityVectors), so the 12 vectors.json frames cannot be reproduced",
    );
  }
  const results = (await (
    consumer as (path: string) => Promise<readonly ParityVectorResult[]>
  )(vectorsUrl.pathname)) as readonly ParityVectorResult[];
  if (results.length !== 12) {
    throw new Error(
      `bt_02: the TS vector consumer asserted ${results.length} vectors, expected 12`,
    );
  }
  for (const [index, vector] of raw.vectors.entries()) {
    if (results[index].frame_hex !== vector.frame.frame_hex) {
      throw new Error(
        `bt_02: vector ${vector.name} frame_hex diverges from the literal in vectors.json`,
      );
    }
    if (
      JSON.stringify(results[index].verdict) !==
      JSON.stringify(vector.expected_verdict)
    ) {
      throw new Error(
        `bt_02: vector ${vector.name} verdict diverges from the expected_verdict literal`,
      );
    }
  }
}

test(
  "bt_02_typescript_vectors_parity_frame_verdict_equality",
  bt_02_typescript_vectors_parity_frame_verdict_equality,
);
