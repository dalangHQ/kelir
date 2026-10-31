- SDK architecture
  - N-native design
    - No dynamic any-typing, no second-class shims
    - All SDKs enforce identical runtime guarantees through native type systems
    - Bootstrap: Rust, TypeScript, Python
    - Adding a language: bindings crate + vector consumer + surface manifest
    - Plugin authorship contract: see: 02-plugin-model.md#Plugin-ownership
    - Two-tier authoring model
      - Native plugins: distributed as precompiled npm/pip native binaries
      - Foreign policy plugins: authored in userland code via native async closures
      - Native vs Foreign: see: 02-plugin-model.md#Dual-plane-execution-architecture
    - Semantic invariant equivalence
      - Runtime safety matrix: see: artifacts/diagrams/sdks-invariants.txt
      - Failure occurs at the earliest lifecycle phase native to each runtime
      - Affine builder consumption: every fluent verb consumes the previous
        builder by value; no builder instance is reused, branched, or mutated
      - Compile-time prerequisite gating: a plugin declaring a prerequisite
        without its dependency installed fails at type-check, not boot
      - Zero dynamic service lookups: handler context c statically intersects
        each installed plugin's surface; no string keys, no runtime downcast
      - Unhandled errors abort hypervisor immediately to eliminate zombie runs
      - Distribution and error invariants
        - Zero runtime toolchain tax: distributed with precompiled NAPI/PyO3 wheels
        - Consumers never require cargo, zig, or C compilers on target systems
        - Unhandled foreign handler error catches cleanly; guest never hangs
        - Declarative error maps convert host exceptions to exact POSIX errnos
        - Unmapped exceptions convert to Decision::Deny or hypervisor abort
        - Structured error envelope carries driver, ring, pid, binary, and errno
    - Filter-before-FFI declaration
      - Declarative filters passed to builder during plugin registration
      - Host core evaluates rules natively; non-matching events skip FFI
    - Type-safety invariants
      - Rust borrow checker and Serde perimeter validation: see: #Rust-SDK
      - TypeScript tsc and TypeBox AOT native validators: see: #TypeScript-SDK
      - Python Pydantic v2 frozen models, import-time checks: see: #Python-SDK
      - NAPI/PyO3 AOT hook fusion: pipeline handlers fuse to one native closure
      - Foreign handlers cross FFI at most once per event
    - Typed dispatch and session extensions
      - Typed event hooks
        - .on(LifecycleEvent.VmReady, handler) for lifecycle events
        - .intercept(Ring.NetOutbound, handler) for ring interception
        - .trap(TrapFilter::default(), handler)
      - Native Engine Planes on Session
        - Core Session directly exposes the 4 planes
        - session.terminal, session.storage, session.wire, session.checkpoint
        - Terminal: session.terminal.resize(120, 40), session.terminal.export_screen_matrix()
          - Replay: session.terminal.read_from(port, offset) -> (Vec<u8>, next_offset)
        - Storage: session.storage.reconcile(), session.storage.discard()
        - Wire: session.wire.forward_port(guest_port, host_port)
          - Returns Result<u16, WireError> with allocated port
        - Checkpoint: session.checkpoint.create(label), session.rollback()
      - Plugin extension pattern
        - Optional domain plugins attach via typed extensions
        - Rust: session.ctx::<T>() infallible via `Session<Plugins, Params>`
          where `Plugins: Has<T>`; no Result, no `ext`/`extension` surface
        - TypeScript: session.<pluginKey> is a static member of
          Session<TContext, TParams>; no session.use(ext)
        - Python: session.cap.<plugin_key> is statically projected by the
          PEP 695 Protocol intersection; no session.ext(T)
        - Inter-plugin capabilities resolve via c.ctx::<T>() (Rust),
          c.<pluginKey> (TS), c.cap.<plugin_key> (Python)
    - Testkit contract: see: 02-plugin-model.md#Plugin-testkit
    - Language surfaces
      - Rust: extension traits provide zero-cost fluent chaining on builder
      - TypeScript: generic context accumulation decorates fluent plugin verbs
      - Python: dynamic protocol mixins inject fluent verbs on mount
      - Chained verbs appear on builder instance only when plugin is mounted
    - Scoping: all SDKs support ring, binary, and path filters at mount point
    - Topology rule: distinct environments use separate profile factories
    - Parity mechanism: see: 15-operations.md#Multi-language-parity-gate
      - SDK naming idiom: same concept, per-SDK spelling; parity is concept parity
        - Sinkhole allowlist (Py/RS) vs dnsExcept (TS)
        - ReceiptConfig: Py/RS include_syscall_count
          - TS: includeDirtyBytes, includeBinaryBlake3, includeSyscallCount
        - Decision static constructors (Py/RS) vs c.deny/c.resume/c.rewrite (TS)
        - Lifecycle decorators/methods (Py/RS) vs .on(LifecycleEvent.VmReady) (TS)
    - Trap verdict surface: see: 11-protocol.md#TrapAction-method-mapping
      - Console interposer signature (all SDKs)
        - on_console_input(port, bytes) / on_console_output(port, chunk)
        - port identifies the virtio-console port; bytes mutated in place
      - Yield surface naming
        - Rust: TrapAction::Yield { checkpoint_id }
        - TS: c.yield(checkpoint_id)
        - Python: Decision.yield_to_host(checkpoint_id)
        - Python has no trailing underscore; yield_to_host is the name
    - Session surfaces
      - Session data operations
        - Shared memory buffers: session.vm.map_shared_region(tag, size_bytes)
        - Portable snapshots: session.vm.export_snapshot() and session.vm.import_snapshot()
      - Session control surface
        - Dynamic RAM ceiling: session.vm.adjust_balloon(target_mb)
        - Dynamic vCPU roster: session.vm.set_vcpus(n)
        - Hard vetoes: LifecycleCtx.register_veto(event, cb); see: 08-lifecycle.md#Lifecycle-vetoes
        - Graceful retirement: session.drain(timeout_ms) -> DrainOutcome; see: 08-lifecycle.md#Hypervisor-primitives
        - Break-glass SSH/SFTP: session.ssh() -> SshHandle; see: 08-lifecycle.md#Break-glass
    - Multiplexed binary RPC for SDK consumers: session.vsock.channel(kind), binary framing
      - Channel kind registration: builder.register_channel(kind, handler)
        - kind drawn from frozen table; see: 11-protocol.md#Channel-allocation-table
      - Programmatic exec surface: session.exec(cmd, limits?)
        - Writes on Channel 0x0003; framing see: 11-protocol.md#Binary-exec-channel
        - Accepts optional ProcessLimits; see: 11-protocol.md#Trap-fast-path
    - Custom vsock RPC services: builder.register_vsock_service(port, schema, handler)
    - Three-tier binary distribution model
      - Dynamic runtimes (TS/Python): Zero Rust compilation by end user; ships precompiled
        NAPI-RS .node and PyO3 .whl native shared libraries
      - Pure Rust enterprise: Gated private Cargo registry (crates.kelir.dev) requiring license
        tokens for on-premise compilation (commercial tier)
      - Kelir Cloud SaaS: Execution over HTTP/gRPC; internal hyper-density and cas-snapshot
        optimization applied transparently on host servers
  - Idiomatic Surfaces Law
    - TS, Python, and Rust do not copy each other's method names
    - Each enforces 1:1 behavioral and wire parity against vectors.json
  - Sandbox high-level facade
    - Pre-composed distribution wrapper bundling KelirBuilder, profile, and tools
    - Initialization via Sandbox.from_config(path) or Sandbox.create(opts)
    - Internal pipeline
      - Mounts plugin-profile to parse declared TOML or YAML specifications
      - Flattens profile into child VirtIO plugins via CompositePlugin
      - Transitions KelirBuilder<Unset> to KelirBuilder<Sealed> at boot
    - Bidirectional synchronization
      - box.export_spec(path) serializes active topology back to disk
      - File hydration leaves builder open for host programmatic hooks
    - Language parity (Sandbox)
      - TypeScript: Sandbox.fromConfig("kelir.toml")
      - Python: Sandbox.from_config("kelir.toml")
      - Rust: Sandbox::from_config("kelir.toml")
    - Facade method delegations
      - box.exec(cmd) routes through virtio-console and trap filters
      - box.rollback() invokes driver rollback: see: artifacts/code/vmm-driver-trait.rs
      - box.reconcile() invokes storage reconciler to host workspace
      - box.suspend() drops host RSS to 0 MB in <50ms
  - Params typed views
    - Host-side typed access on HandlerCtx
      - Rust: c.params::<T>() deserializes from validated buffer
      - TypeScript: params typed by .params(schema) (TypeBox)
      - Python: params typed by Pydantic BaseModel
    - Zero-allocation validation: operates on zero-copy guest memory
  - Rust SDK
    - see: artifacts/code/rust-handle-injection.rs
    - Typestate transitions: KelirBuilder<Unsealed, Plugins, Params> to
      KelirBuilder<Sealed, Plugins, Params> to Session<Plugins, Params>
    - Every verb consumes self by value: .plugin(), .params(), .trap(), .boot()
    - .detached(true) spawns unowned runtime child registered in SQLite sessions
    - Relay endpoint binds to ~/.kelir/sessions/<id>/relay.sock
      - see: 08-lifecycle.md#Detached-lifecycle
    - Reattachment via session.attach(session_id) returns Session
    - Plugins accumulate in a monomorphic HList; HandlerCtx<'a, Plugins, Params>
      resolves capabilities via Has<T>, infallible at the call site
    - .plugin() takes flat plugin instances; .params::<T>() carries the schema
    - .plugin() accepts core KelirPlugin plus only needed ring traits
    - Ring traits: LifecycleParticipant, TrapInterposer, ConsoleInterposer
    - FsInterposer and NetInterposer are synchronous hot-path traits
    - AnyPlugin: identity marker for any KelirPlugin, not a capability bundle
    - Example trait bounds follow the split; unneeded traits stay unimplemented
    - .trap(matcher, handler): pre-exec gate; dual pre/post handler also supported
    - .intercept_storage(pathGlob, handler): synchronous native storage write filter
    - .on_vm_ready(handler): session callback; .boot().await seals and boots
    - Inter-plugin capabilities resolve via c.ctx::<T>() where Plugins: Has<T>;
      a missing plugin is a compile error, never a runtime Result
    - Composite bundles flatten into builder via CompositePlugin trait
    - Scope constraint via MountOptions: rings, binaries, paths
  - TypeScript SDK
    - see: artifacts/code/typescript-accumulation.ts
    - KelirBuilder<TContext, TParams, TInstalledPlugins> carries the accumulated
      surface in type parameters; every verb returns a new builder type
    - .use(plugin) intersects TPluginCtx into TContext and unions TPluginId
      into TInstalledPlugins; .use() takes flat plugin instances, no callbacks
    - Prerequisite gate: a missing dependency yields
      CompileError<"Missing ...">, so no method (including .params and .trap)
      is callable on the returned type
    - .trap(matcher, handler): simple pre-exec gate returning a Decision
    - .trap(matcher, { pre, post }): dual pre/post gate; post returns a
      PostExecAction
    - .intercept(channel, filter, handler): ring-level synchronous hot-path
      filters returning SyncVerdict
    - .on(event, handler): lifecycle interceptors over LifecycleEvent values
    - .onTrap(handler): plugin-level trap interposer
      - Returns a new plugin; the plugin owns on_trap for its binaries
      - Builder .trap() remains for one-off gates without a plugin
    - .params(schema) pins TParams = Static<TSchema>; handler c.params is typed
      from that parameter, not from a runtime schema lookup
    - Handlers receive HandlerContext<TContext, TParams>; plugin surfaces are
      static members (c.policy, c.ledger), never c.service(Token)
    - AOT hook fusion compiles all TS pipeline handlers into a single flat closure
    - Invariant: exactly 1 NAPI trampoline jump per event regardless of depth
    - Fully unified TContext passes to callbacks; cross-plugin reads are static
      member accesses, not service-token resolution
    - Wraps async TS handlers in ForeignPluginProxy at NAPI boundary
    - Null-object pattern: use(plugin({ enabled: false })) preserves static
      member shape with a no-op implementation
  - Python SDK
    - see: artifacts/code/python-raii-pydantic.py
    - async with builder as session
      - __aenter__ boots microVM
      - __aexit__ guarantees session.terminate() on any exception
    - KelirBuilder[TCapabilities, TParams] carries the accumulated surface in
      PEP 695 type parameters; every verb returns a new builder type
    - .use(plugin) intersects TNewCap into TCapabilities via Protocol
      intersection; the c.cap projection is statically typed by pyright/mypy
    - .params(schema: type[TNewParams]) pins TParams; handler c.params is typed
      from the Pydantic model class
    - Composite plugins unpack nested plugins atomically before boot
    - Capabilities are static members of c.cap (c.cap.policy), never
      c.service(ServiceType)
    - Mount scoping via kwargs: rings=..., binaries=..., paths=...
  - Full-surface examples
    - see: artifacts/examples/autonomous-swe-agent.rs
    - see: artifacts/examples/autonomous-swe-agent.ts
    - see: artifacts/examples/autonomous-swe-agent.py
    - see: artifacts/examples/multi-vm-swarm.rs
    - see: artifacts/examples/serverless-webhook.ts
    - see: artifacts/examples/db-backed-runtime.py
    - Position: integration examples, not "the" example
      - Each plugin's own tests/ and examples/ are canonical for that plugin
      - These three demonstrate composition across plugins and rings
      - The additional three demonstrate the primitives and plugins
        that the SWE-bench family does not mount
    - Each example exercises
      - Its declared plugin set
        - Plugin-set registry: see: 02-plugin-model.md#First-class-plugins
      - Every VirtIO ring those plugins attach to
      - The evidence tiers and primitives it names
    - Not asserted per example
      - Every ring
      - Every plugin
      - Every primitive
      - Coverage is per-plugin, not per-example
    - Not transliterations: each uses language-idiomatic constructs
      - Rust: async-trait, JoinSet, Arc<Mutex<>> states
      - TypeScript: Promise.all fan-out, generic inference
      - Python: asyncio.gather, async with, Pydantic v2
