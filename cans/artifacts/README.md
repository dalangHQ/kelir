# artifacts/

Terminal references for the kelir CANS spec in `../`.

## What lives here

| Directory | Contents |
|---|---|
| `code/` | Standalone source files extracted from the spec: traits, structs, protocol definitions, SDK usage examples |
| `config/` | Build and CI configuration: justfile, GitHub Actions workflow, project tree |
| `diagrams/` | ASCII architecture diagrams that do not fit cleanly as CANS nodes |
| `examples/` | Full-surface SDK consumer examples (moved from repo root during CANS migration) |

## What does not live here

- CANS spec nodes — those are in `../`
- Anything with an outbound reference — artifacts are terminal by design

## Convention

Every artifact is referenced from `cans/` via `see:` with a **relative** path:

```
cans/01-architecture.md       →  see: artifacts/code/vmm-driver-trait.rs
cans/04-terminal-plane.md     →  see: artifacts/diagrams/terminal-pipeline.txt
cans/03-sdk-architecture.md   →  see: artifacts/examples/autonomous-swe-agent.rs
```

- `cans check` treats every artifact pointer as a **0-hop** reference
- Artifacts are never spec-edited during a CANS migration; they are frozen reference material
- An artifact may contain inline file-level comments pointing back to canonical spec nodes, but the artifact itself issues no CANS reference
- One artifact may be referenced from multiple spec nodes (no duplication)

## Editing rules

1. Never edit a spec concept inside an artifact — move the concept to a CANS node and reference it
2. Never duplicate a spec concept across multiple artifacts
3. Never create an artifact with no inbound `see:` — orphans are dead weight
4. An artifact that is not verbatim source (e.g. an SDK usage excerpt) is marked with a top-of-file comment stating it is illustrative, not standalone-compilable
