- Plugin tool-guard
  - Crate dependencies (tool-guard)
    - Plugin identifier: plugin-tool-guard
    - Owner crate: crates/kelir-plugins/plugin-tool-guard
    - name() = "tool-guard"
    - Tier 2 governance and protocol proxy (tool-guard)
    - Depends on (tool-guard): kelir-interface, plugin-mcp
    - Imports plugin-mcp AST and call definitions directly
    - Exposes: ToolRegistryService
    - VirtIO device attachment (tool-guard): see: 01-architecture.md#VirtIO-Device-Complex
    - Lifecycle attachment (tool-guard): see: 07-lifecycle.md#Lifecycle-vetoes
    - Executes upstream of plugin-mcp on the MCP console port
  - Service exports (tool-guard)
    - Exports ToolRegistryService via PipelineBinder for schema and approval lookups
  - Invariants and policy (tool-guard)
    - Total tool-call interposition
      - Intercepts all JSON-RPC 2.0 frames on the MCP console port matching "tools/call"
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
      - Mount via builder.use(toolGuard()) decorates builder with .tool()
      - Rust: exposes ToolGuardExt extension trait for direct builder chaining
      - Python: registers @builder.on_tool decorator dynamically on mount
      - Non-matching tool calls evaluate static MountScope allowlists;
        matching calls cross FFI trampoline to host
    - Decision lattice (tool-guard)
      - ToolAction::Allow: forwards frame untouched
      - ToolAction::Rewrite(mutation): serializes canonical ExecveMutation ABI:
        - new_binary: Option<String> (overrides target ELF path)
        - new_argv: Option<Vec<String>> (replaces execution arguments)
        - set_env: HashMap<String, String> (injected environment variables)
        - unset_env: Vec<String> (stripped environment variables)
        Packed into target_mutation bytes; guest shim decodes and applies
        atomically before releasing trapped execve syscall
      - ToolAction::Deny(errno, reason): drops call, synthesizes standard JSON-RPC error response
      - ToolAction::Hold: requests hypervisor
        LifecycleRequest::Suspend(External)
        - Swap mechanics: see: 07-lifecycle.md#Suspend-and-resume
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
