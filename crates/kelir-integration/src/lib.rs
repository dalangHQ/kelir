// see: cans/15-operations.md#Per-plugin-CI
#![forbid(unsafe_code)]

// L3-GAP: [hypervisor_driver]: tests/test_full_pipeline.rs + tests/test_fork_fanout.rs per cans/artifacts/config/project-structure.txt.; slop: pipeline tests assert Ok(()), fast-boot surface unexercised
// see: cans/02-plugin-model.md#KelirPlugin-trait
// L3-GAP: [plugin_engine]: fluent builder wrapper coverage once the engine binds builder wrappers per cans/02-plugin-model.md#KelirPlugin-trait.; slop: wrapper coverage claimed, engine binding never exercised
