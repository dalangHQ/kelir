- Snapshots
  - Snapshot taxonomy
    - Restore modes
      - Branch-from-any-depth
        - Restore accepts any snapshot digest in the chain, not only the leaf
        - The runtime materializes the full ancestor prefix as read-only layers
        - The restored session's working delta becomes a new child of that digest
        - Ancestors remain shareable by later restores; CoW page sharing
          applies across all concurrent children of the same ancestor
        - Depth is unbounded; there is no depth-1 StepSlot ceiling
  - Archives and portability
    - msb archives bundle snapshots into portable .msb files
      - Unpack invariants (see: 07-lifecycle.md#Portable-snapshot-bundles)
        - Extraction is contained to the staging root via openat2 RESOLVE_BENEATH
        - Sparse entries pre-sized; zstd frames bounded by declared size
        - Traversal, absolute paths, and symlink escapes rejected before extraction
        - Codec errors discard staging atomically; no partial roots leak
  - Groups and lineage
    - Lineage tracks parent child edges for garbage collection
      - Every snapshot row carries `parent_digest` and `child_digests`
      - Chains are unbounded; depth is a property of the reachability graph
        rather than a schema cap
      - Compaction may fold interior nodes when a branch is retired; parent
        and child digests are rewritten atomically inside a SQLite transaction
      - A live head pins its full ancestor prefix; interior nodes with multiple
        live descendants remain materialized