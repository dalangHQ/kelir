- Storage catalog schema
  - Entity model
    - Snapshot rows link artifacts to their lineage group
      - Snapshot rows carry `parent_digest` and, on interior nodes,
        `child_digests` as an indexed JSON array
      - Chain depth is unbounded; reachability walks handle traversal
      - `guest_clock` is stored as JSON (absolute_wall_clock_ns, monotonic_offset_ns,
        timezone, boot_params) on every captured snapshot
      - Runtime rows carry an explicit `maintenance_lease` link for detached
        sessions; TTL expiry reclaims