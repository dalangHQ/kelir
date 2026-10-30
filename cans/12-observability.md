- Observability
  - Log streaming
    - Storage and cursor semantics are canonical in 04-terminal-plane.md#Stream-replay
    - The observability layer is a thin consumer
      - Every chunk is tagged with the (session_id, stream_id, cursor) triple
      - stream_id: 1=stdout, 2=stderr, 3=pty, per exec_id when applicable
      - Chunks land in flat files under sessions/<id>/logs/; indexed in SQLite
      - No wrap-around loss; durable flat files
    - Client contract
      - Requests take `since_cursor` (optional, nullable for tail)
      - Replays are idempotent; same cursor returns the same bytes
      - `follow=true` streams new chunks; `follow=false` returns once
      - Guarantee spans reconnect, process restart, and CLI reattach
    - Watchers
      - Multiple concurrent readers fan out cleanly; no locks, no drops
      - stream delivery has its own e2e pair
      - attach_output proves capture during attach
      - msb logs follows the same convention; thin wrapper
    - Shutdown
      - Drain (see: 07-lifecycle.md#Hypervisor-primitives) flushes logs before termination
      - Abrupt kill may leave logs truncated at last flushed chunk; the
        cursor remains monotonic and replay-safe
  - Metrics pipeline
    - A shared registry exports machine readable counters
      - Exporters ship to stdout or an otel compatible endpoint
      - Compatibility views keep older dashboard queries working
    - OTLP is the default export surface
      - Native gRPC and HTTP exporters via OpenTelemetry SDK
      - Metrics: counters (execs, syscalls, drops, backpressure events),
        gauges (RSS, dirty pages, active vCPUs, in-flight execs)
      - Traces: one span per receipt phase (trap_ingress, policy, ffi_hop,
        verdict_egress); see: 08-epistemic.md#Execution-receipts
      - Trace continuity across guest-originated requests via W3C traceparent
      - No local Merkle WAL is written by default; see: plugins/ledger.md
    - Host dashboards (Prometheus, Datadog, Grafana Alloy, OTel Collector)
      consume without bespoke scrapers
  - Health signals
    - Heartbeat loss marks a guest as stalled or wedged
    - Parent watchdogs reap orphaned runtime processes
      - Watchdog lives in the runtime child; reads the SQLite lease
        (see: 01-architecture.md#Host-state-root)
      - Lease heartbeat from the runtime proves liveness; TTL expiry
        triggers the janitor
      - runtime/reap and sandbox/reap implement the sweep
      - child_reaping checks the process tree
      - finished_ids remember already-done work
    - Correlation ids stitch spans across cli, runtime, agentd