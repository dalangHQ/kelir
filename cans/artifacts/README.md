# artifacts/

Terminal content for the kelir CANS spec in `../`.

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

## SSOT contract

Artifacts and nodes split truth by kind, not precedence.

- Artifacts own **content**: enumerations, field sets, code signatures,
  topology, timing constants, wire layouts.
- Nodes own **normativity**: ordering, placement, threat model, invariants,
  and the rationale for each.
- A node that re-lists artifact content is a defect. The node must say
  something the artifact cannot — otherwise it is decoration.
- The test: *can the node articulate an invariant the artifact does not
  encode?* If not, the node is wrong, not the artifact.

## Convention

Every artifact is referenced from `cans/` via `see:` with a **relative** path
and is accompanied by a node-level invariant:

```
cans/01-architecture.md       →  see: artifacts/code/vmm-driver-trait.rs
                                  invariant: the trait is the sole hypervisor boundary
cans/04-terminal-plane.md     →  see: artifacts/diagrams/terminal-pipeline.txt
                                  invariant: filter ordering (see #Five-filters)
cans/03-sdk-architecture.md   →  see: artifacts/examples/autonomous-swe-agent.rs
                                  invariant: same plugin set across all three SDKs
```

- `cans check` treats every artifact pointer as a **0-hop** reference
- Artifacts are never spec-edited; they are frozen content
- An artifact may carry a file-header pointer back to its owning node; the
  artifact issues no CANS reference
- One artifact may be referenced from multiple spec nodes (no duplication)

## Editing rules

1. Edit content in the artifact; edit invariants in the node. Never cross.
2. Never duplicate artifact content in a node.
3. Never create an artifact with no inbound `see:` carrying a node-level
   invariant.
4. Non-verbatim source (SDK excerpts, illustrative snippets) is marked at the
   top of the file as illustrative, not standalone-compilable.
