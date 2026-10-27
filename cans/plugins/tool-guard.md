- Plugin tool-guard
  - Capability definition (tool-guard)
    - Plugin identifier: plugin-tool-guard
    - Owner crate: crates/kelir-plugins/plugin-tool-guard
    - name() = "tool-guard"
    - Taxonomy (tool-guard): urn:kelir:io:mcp:tool
      - coordinate: rings=[virtio-console:hvc1]; role=Gate; tier=Native
    - Capabilities (tool-guard):
      - provides = [Capability("io:mcp:tool-guard")]
      - requires = []
      - order = [OrderConstraint::RunBefore(Capability("io:channel:mcp"))]
    - VirtIO device attachment (tool-guard): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (tool-guard): see: 07-lifecycle.md#Lifecycle-vetoes
    - Topologically executes upstream of plugin-mcp on virtio-console hvc1
  - Service exports (tool-guard)
    - Exports ToolRegistryService via PipelineBinder for schema and approval lookups
  - Invariants and policy (tool-guard)
    - Total tool-call interposition
      - Intercepts all JSON-RPC 2.0 frames on hvc1 matching method "tools/call"
      - Deserializes tool name and arguments in native memory before execution
      - Interposition is complete: no tool invocation reaches guest
        execution shims without passing tool-guard
    - Anti-Rug-Pull schema pinning
      - Intercepts initial "tools/list" advertisement at on_vm_ready
      - Computes BLAKE3 hash over all tool schemas, signatures, and descriptions
      - Locks the catalog: dynamic schema mutation or description injection
        post-boot halts with ESecurityViolation
    - Host extensible hook pipeline
      - Exposes typed pre-call and post-call interception to host runtimes
      - Host registers closures via SDK: builder.intercept_tool(tool_name, handler)
      - Non-matching tool calls evaluate static MountScope allowlists;
        matching calls cross FFI trampoline to host
    - Decision lattice (tool-guard)
      - ToolAction::Allow: forwards frame untouched
      - ToolAction::Rewrite(args): mutates payload before dispatch
        (e.g. forced flags, path sandboxing)
      - ToolAction::Deny(errno, reason): drops call, synthesizes standard JSON-RPC error response
      - ToolAction::SuspendForApproval: requests hypervisor
        LifecycleRequest::Suspend(HostHold(TOOL_GUARD_HOLD))
        - Freezes vCPU execution and flushes guest RAM to NVMe (0 MB host RSS)
        - Host waits for human out-of-band authorization via vsock Channel 0
    - "Denial of Wallet" recursion limiter
      - Per-tool token-bucket governor clamps call cadence
      - Call depth ceiling: max_invocations_per_trace (default: 50)
      - Infinite recursion or retries immediately reject with ECYCLICREWRITE / ETOOLQUOTA
    - Parameter & payload sanitization
      - Zero-copy regex & Aho-Corasick scanning over string arguments
      - Injects vault:// placeholder dereferencing: raw secrets in arguments reject immediately
      - Output scrubber: scans tool execution responses on return;
        strips host secrets before model context ingests bytes
  - Error and return semantics (tool-guard)
    - Hook signature (tool-guard): see: artifacts/code/plugin-trait.rs
    - Return errno and rejection behavior (tool-guard)
      - ToolAction::Deny returns JSON-RPC error object:
        - code = -32001 (EPERM) or mapped POSIX errno
        - message = user-defined reason or "Policy violation: tool execution rejected"
      - Schema drift or unpinned tool tampering returns -32601
        (Method not found) + VM Security Panic
