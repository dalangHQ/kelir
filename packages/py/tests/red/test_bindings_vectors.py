# see: cans/artifacts/test-plan/cases/bindings_py/bp_02_python_vectors_parity_frame_verdict_equality.yaml#case_id
from __future__ import annotations

import json
from pathlib import Path

import kelir

VECTORS_PATH = Path(__file__).resolve().parents[4] / "vectors.json"


def bp_02_python_vectors_parity_frame_verdict_equality() -> None:
    vectors = json.loads(VECTORS_PATH.read_text())["vectors"]
    assert len(vectors) == 12, f"bp_02: vectors.json carries {len(vectors)} vectors, expected 12"
    assert vectors[0]["name"] == "trap-git-commit-pass", (
        "bp_02: vectors.json first vector is "
        f"{vectors[0]['name']}, expected trap-git-commit-pass"
    )
    consumer = getattr(kelir, "run_parity_vectors", None)
    if consumer is None:
        raise AssertionError(
            "bp_02: the Python SDK exports no parity vector consumer "
            "(kelir.run_parity_vectors), so the 12 vectors.json frames cannot be reproduced"
        )
    results = consumer(str(VECTORS_PATH))
    assert len(results) == 12, (
        f"bp_02: the Python vector consumer asserted {len(results)} vectors, expected 12"
    )
    for vector, result in zip(vectors, results):
        assert result["frame_hex"] == vector["frame"]["frame_hex"], (
            f"bp_02: vector {vector['name']} frame_hex diverges from the literal in vectors.json"
        )
        assert result["verdict"] == vector["expected_verdict"], (
            f"bp_02: vector {vector['name']} verdict diverges from the expected_verdict literal"
        )


def test_bp_02_red() -> None:
    bp_02_python_vectors_parity_frame_verdict_equality()
