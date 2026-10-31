- Observability
  - Log streaming
    - Storage, cursor, and reconnect semantics: see: 04-terminal-plane.md#Stream-replay
    - Observability layer acts as a thin reader over flat log index files
    - Watchers
      - Multiple concurrent readers fan out cleanly; no locks, no drops
      - stream delivery has its own e2e pair
      - attach_output proves capture during attach
      - kelir logs follows the same convention; thin consumer of index.bin
    - Shutdown
      - Drain flushes logs before termination; see: 08-lifecycle.md#Hypervisor-primitives
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
        verdict_egress); see: 10-epistemic.md#Execution-receipts
      - Trace continuity across guest-originated requests via W3C traceparent
      - No local Merkle WAL is written by default; see: plugins/ledger.md
    - Host dashboards (Prometheus, Datadog, Grafana Alloy, OTel Collector)
      consume without bespoke scrapers
  - Health signals
    - Heartbeat loss marks a guest as stalled or wedged
    - Parent watchdogs reap orphaned runtime processes
      - Watchdog in runtime child reads SQLite lease; see: 01-architecture.md#Host-state-root
      - Lease heartbeat from the runtime proves liveness; TTL expiry
        triggers the janitor
      - OS kernel death signals terminate backends on host parent exit
      - Runtime child janitor sweeps expired session directories on startup
    - Correlation ids stitch spans across cli, runtime, agentd