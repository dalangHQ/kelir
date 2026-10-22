- Protocol
  - Control channel
    - virtio-vsock port 5252: control/trap plane
    - virtio-vsock port 5253: ingress/port-forward multiplexer
    - Windows driver-openvmm: HV_SOCKET translates the same channels
    - Zero network interface involvement
    - Pluggable host listener registration via register_vsock_service()
      - Maps custom vsock ports (e.g. 5254) to host async stream callbacks
      - Provides direct host RPC channel without external network routing
    - Port collision across co-located sessions: TBD
  - Trap fast-path
    - Step sequence and per-step mechanics: See artifacts/diagrams/trap-flow.txt
    - Goal: the trap path must not slow `cargo build` / `npm install`
    - Why three layers, not one
      - A single trap-on-every-syscall design adds ~µs per fork/exec at
        toolchain scale; the allowlist exists to pay that cost only where
        policy is possible
      - A userspace-only inspection cannot close the check-to-use race;
        the freeze exists to make the metadata binding
      - A frozen inspection is still blind to what the loader *will* do;
        the linker audit exists to make the inspection meaningful
    - Layer cost if removed
      - Layer 1 removal: every rustc invocation round-trips to the host
      - Layer 2 removal: sibling threads rewrite memory during inspection;
        inspection is decorative
      - Layer 3 removal: LD_PRELOAD rootkits and encrypted argv payloads
        pass through untouched
    - Measurement rule
      - Entropy is over argv[1:]: including argv[0] dilutes the signal with
        a low-entropy path
      - Flag threshold: Shannon entropy > 5.2 flags encrypted or base64
        shell payloads
      - Allowlist bypass: entropy_bypass_paths skips check for trusted runners
      - Permits high-entropy base64 arguments for node, python, and runners
      - ELF is read from disk at trap time: /proc/$PID/maps still describes
        the pre-exec image and cannot be trusted
    - Flow diagram: See artifacts/diagrams/trap-flow.txt
    - Fast-path policy: TBD
  - Wire framing
    - See: artifacts/code/wire-protocol.rs
    - Layout (header offsets, magic, version): See artifacts/code/wire-protocol.rs
    - Framing invariants
      - One frame = one message; no message spans frames
      - Sequence is monotonic per vsock connection and shared across both
        directions; a lost seq is a protocol error, not a retry
      - Payload is CBOR; the codec never inspects CBOR before reassembling
        the u64 sequence
    - Framing gaps: TBD
      - Byte order
      - Message-type registry
      - Flags registry
      - Max frame size
      - CBOR limits
      - Version negotiation
      - Flow control
      - Reconnect and sequence reset
      - Port 5253 framing
  - Wire messages
    - See: artifacts/code/messages.rs
    - Message set and payload fields: See artifacts/code/messages.rs
    - Hook binding: one hook per message; see 02-plugin-model.md#KelirPlugin-trait
    - TrapEvent carries optional ambient 64-bit task_token for subagent threads
    - TrapAction invariants
      - `Allow` cannot mutate registers, memory, or env; it is a no-op
      - `Yield { checkpoint_id }` freezes vCPU registers and CoW memory
      - `Yield` returns execution cleanly to host event loop without polling
      - `Abort { exit_code, reason }` executes uncatchable hypervisor SIGKILL
      - `Abort` halts execution immediately on critical security violations
      - `Deny` injects an exit code and stderr into the agent TTY
      - `Rewrite` is the only variant that can carry injected env; the
        tty_notice field routes through HostToGuest::Disclosure
      - `SuspendForApproval { poll_interval_ms }` parks the trap; released
        only by Oracle.await_human_approval()
      - `Custom` variants are dispatched by name; unknown names fail with
        EUnknownAction before the pipeline resumes
  - Env injection
    - Allow cannot mutate guest environment
      - Registers and memory untouched
      - A plain Allow decision is by definition a no-op
    - Handler-side c.env.set(...) reaches guest only through Rewrite
    - Auto-Rewrite on resume
      - Handler calls c.env.set(...) then returns Decision.resume()
      - Runtime auto-converts to TrapAction::Rewrite
      - new_binary = original_binary
      - new_argv = original_argv
      - injected_env = accumulated handler env map
      - tty_notice = handler's tty_notice if set, else None
      - Syscall re-executes through Rewrite path with injected env
    - This is the only channel for handler-side env changes to reach guest
  - Trap decision gaps: TBD
    - Rewrite loop prevention
    - new_binary canonicalisation
    - Env injection size and secrets
    - Multi-handler env merge
    - Custom dispatch
    - SuspendForApproval release
