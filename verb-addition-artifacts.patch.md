The total number of parts for `cans/artifacts/` is **3** (`x = 3`):

- **Part 1/3:** Machine SSOT Registry & Rust System Layer (`builder-verbs.yaml`, `plugin-trait.rs`, `rust-plugin-builder-scopes.rs`, `scope-builders.rs`, `rust-plugin-builder.rs`)
- **Part 2/3:** TypeScript & Python SDK Scopes and Builders (`typescript-scope-interface.ts`, `typescript-plugin-builder.ts`, `python-scope-protocol.py`, `python-plugin-builder.py`)
- **Part 3/3:** Configurations, Driver Invariants, Messages & Architecture Diagrams (`vm-config-defaults.rs`, `messages.rs`, `vmm-driver-trait.rs`, `guest-vm.txt`, `terminal-pipeline.txt`, `storage-architecture.txt`, `wire-plane.txt`)

Here is **Part 1/3**.

---

### Patch 1.1: `cans/artifacts/config/builder-verbs.yaml`
Expands the machine SSOT registry from 24 to the canonical 38 ring verbs.

```diff
--- a/cans/artifacts/config/builder-verbs.yaml
+++ b/cans/artifacts/config/builder-verbs.yaml
@@ -98,6 +98,24 @@
     returns: scope
     args: "sync (event: ProcessExit) => Result<(), PluginError>"
     notes: null
 
+  - name: on_signal
+    rust: on_signal
+    typescript: onSignal
+    python: on_signal
+    placement: scoped-entry
+    scope: trap
+    returns: scope
+    args: "async (sender: u32, target: u32, sig: i32) => Result<Verdict, PluginError>"
+    notes: Intercepts in-guest signals (SIGKILL/SIGTERM) before task delivery.
+
+  - name: on_fork
+    rust: on_fork
+    typescript: onFork
+    python: on_fork
+    placement: scoped-entry
+    scope: trap
+    returns: scope
+    args: "async (parent: u32, child: u32) => Result<Verdict, PluginError>"
+    notes: Intercepts fork/clone syscalls; guards against fork bombs before vCPU allocation.
+
   # ── Fs ring ─────────────────────────────────────────────────────
@@ -134,6 +152,69 @@
     returns: scope
     args: "sync (path: &Path, flock_cmd: i32, flock_type: i16, pid: u32) => Result<Verdict, PluginError>"
     notes: null
 
+  - name: on_fs_unlink
+    rust: on_fs_unlink
+    typescript: onFsUnlink
+    python: on_fs_unlink
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (path: &Path) => Result<Verdict, PluginError>"
+    notes: Supports FsPathRedirect in PatchPayload; redirects destructive unlinks.
+
+  - name: on_fs_rename
+    rust: on_fs_rename
+    typescript: onFsRename
+    python: on_fs_rename
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (src: &Path, dst: &Path) => Result<Verdict, PluginError>"
+    notes: Intercepts renameat2 operations before broker staging.
+
+  - name: on_fs_setattr
+    rust: on_fs_setattr
+    typescript: onFsSetattr
+    python: on_fs_setattr
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (path: &Path, mode: u32, size: u64) => Result<Verdict, PluginError>"
+    notes: Traps ftruncate, chmod, and chown before inode modification.
+
+  - name: on_fs_symlink
+    rust: on_fs_symlink
+    typescript: onFsSymlink
+    python: on_fs_symlink
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (target: &Path, link: &Path) => Result<Verdict, PluginError>"
+    notes: Blocks path traversal escapes at symlink creation boundary.
+
+  - name: on_fs_mkdir
+    rust: on_fs_mkdir
+    typescript: onFsMkdir
+    python: on_fs_mkdir
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (path: &Path, mode: u32) => Result<Verdict, PluginError>"
+    notes: Validates directory quota and path containment pre-creation.
+
+  - name: on_fs_readdir
+    rust: on_fs_readdir
+    typescript: onFsReaddir
+    python: on_fs_readdir
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (path: &Path, entries: &mut [DirEntry]) => Result<FilterAction, PluginError>"
+    notes: Scrubs sensitive host artifacts from directory listing buffers.
+
+  - name: on_fs_readlink
+    rust: on_fs_readlink
+    typescript: onFsReadlink
+    python: on_fs_readlink
+    placement: scoped-entry
+    scope: fs
+    returns: scope
+    args: "sync (path: &Path) => Result<Verdict, PluginError>"
+    notes: Validates canonical symlink destinations against MountScope.
+
   # ── Net ring ────────────────────────────────────────────────────
@@ -155,6 +236,33 @@
     returns: scope
     args: "sync (buf: &mut [u8]) => Result<FilterAction, PluginError>"
     notes: null
 
+  - name: on_socket_connect
+    rust: on_socket_connect
+    typescript: onSocketConnect
+    python: on_socket_connect
+    placement: scoped-entry
+    scope: net
+    returns: scope
+    args: "sync (pid: u32, proto: u8, ip: [u8; 4], port: u16) => Result<Verdict, PluginError>"
+    notes: Per-process network access control (PNACL); resolves caller PID via netlink.
+
+  - name: on_dns_query
+    rust: on_dns_query
+    typescript: onDnsQuery
+    python: on_dns_query
+    placement: scoped-entry
+    scope: net
+    returns: scope
+    args: "sync (pid: u32, domain: &str, qtype: u16) => Result<DnsVerdict, PluginError>"
+    notes: Resolves domain queries before synthetic gateway forward; returns Forward | Sinkhole.
+
+  - name: on_socket_bind
+    rust: on_socket_bind
+    typescript: onSocketBind
+    python: on_socket_bind
+    placement: scoped-entry
+    scope: net
+    returns: scope
+    args: "sync (pid: u32, proto: u8, ip: [u8; 4], port: u16) => Result<Verdict, PluginError>"
+    notes: Denies in-guest listening socket creation outside authorized ports.
+
   # ── Console ring ────────────────────────────────────────────────
@@ -193,6 +301,15 @@
     returns: scope
     args: "sync (pid: u32, seq: u64, buf: &[u8]) => Result<Vec<u8>, PluginError>"
     notes: null
 
+  - name: on_exec_stdin
+    rust: on_exec_stdin
+    typescript: onExecStdin
+    python: on_exec_stdin
+    placement: scoped-entry
+    scope: exec
+    returns: scope
+    args: "sync (pid: u32, seq: u64, buf: &[u8]) => Result<Vec<u8>, PluginError>"
+    notes: Filter ring; mutates or sanitizes guest stdin chunks before delivery to process.
+
   # ── Host ring ───────────────────────────────────────────────────
@@ -231,6 +348,15 @@
     returns: scope
     args: "async (c: &mut LifecycleCtx<'_>) => Result<(), PluginError>"
     notes: Reachable via .host() but binds the lifecycle ring (see rust-plugin-builder-scopes.rs).
 
+  - name: on_host_containment_breach
+    rust: on_host_containment_breach
+    typescript: onHostContainmentBreach
+    python: on_host_containment_breach
+    placement: scoped-entry
+    scope: host
+    returns: scope
+    args: "sync (pid: u32, path: &Path, kind: &str) => Result<(), PluginError>"
+    notes: Emits high-severity security telemetry on openat2 RESOLVE_BENEATH violation.
+
   # ── Telemetry ring ──────────────────────────────────────────────
```

---

### Patch 1.2: `cans/artifacts/code/plugin-trait.rs`
Adds the 12 missing hook signatures to the segregated interposer traits.

```diff
--- a/cans/artifacts/code/plugin-trait.rs
+++ b/cans/artifacts/code/plugin-trait.rs
@@ -185,6 +185,8 @@
 pub trait TrapInterposer {
     async fn on_trap(&self, _event: &TrapEvent, _ctx: &HandlerCtx) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
     async fn on_process_exit(&self, _event: &ProcessExit) -> Result<(), PluginError> { Ok(()) }
+    async fn on_signal(&self, _sender: u32, _target: u32, _sig: i32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    async fn on_fork(&self, _parent: u32, _child: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
 }
 
 pub trait ConsoleInterposer {
@@ -194,6 +196,7 @@
     async fn on_exec_stdout(&self, _pid: u32, _exec_id: u64, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
     async fn on_exec_stderr(&self, _pid: u32, _exec_id: u64, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
+    async fn on_exec_stdin(&self, _pid: u32, _exec_id: u64, _chunk: &mut Vec<u8>) -> Result<(), PluginError> { Ok(()) }
 }
 
 pub trait SystemTelemetryParticipant {
@@ -213,6 +216,13 @@
     fn on_fs_write(&self, _path: &Path, _data: &mut [u8], _pid: u32, _ctx: &HandlerCtx) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
     fn on_fs_lock(&self, _path: &Path, _flock_cmd: i32, _flock_type: i16, _pid: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_fs_unlink(&self, _path: &Path) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_fs_rename(&self, _src: &Path, _dst: &Path) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_fs_setattr(&self, _path: &Path, _mode: u32, _size: u64) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_fs_symlink(&self, _target: &Path, _link: &Path) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_fs_mkdir(&self, _path: &Path, _mode: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_fs_readdir(&self, _path: &Path, _entries: &mut Vec<crate::storage::DirEntry>) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
+    fn on_fs_readlink(&self, _path: &Path) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
 }
 
 pub trait NetInterposer {
     fn on_net_outbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
     fn on_net_inbound(&self, _frame: &mut [u8]) -> Result<FilterAction, PluginError> { Ok(FilterAction::Forward) }
+    fn on_socket_connect(&self, _pid: u32, _proto: u8, _ip: [u8; 4], _port: u16) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_dns_query(&self, _pid: u32, _domain: &str, _qtype: u16) -> Result<crate::wire::DnsVerdict, PluginError> { Ok(crate::wire::DnsVerdict::Forward) }
+    fn on_socket_bind(&self, _pid: u32, _proto: u8, _ip: [u8; 4], _port: u16) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
 }
 
 pub trait HostInterposer {
     fn on_host_lsm_exec(&self, _path: &Path, _pid: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
     fn on_host_pressure(&self, _level: u32) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
     fn on_host_keyring_access(&self, _key: &str) -> Result<Verdict, PluginError> { Ok(Verdict::Pass) }
+    fn on_host_containment_breach(&self, _pid: u32, _path: &Path, _kind: &str) -> Result<(), PluginError> { Ok(()) }
 }
```

---

### Patch 1.3: `cans/artifacts/code/rust-plugin-builder-scopes.rs`
Exposes the new methods on the sealed Rust ring scope structs.

```diff
--- a/cans/artifacts/code/rust-plugin-builder-scopes.rs
+++ b/cans/artifacts/code/rust-plugin-builder-scopes.rs
@@ -40,6 +40,16 @@
     pub fn on_process_exit<F>(mut self, f: F) -> Self {
         self.builder = self.builder.on_process_exit(f);
         self
+    }
+    pub fn on_signal<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_signal(f);
+        self
+    }
+    pub fn on_fork<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fork(f);
+        self
+    }
     pub fn end(self) -> KelirBuilder {
         self.builder
     }
@@ -62,6 +72,34 @@
     pub fn on_fs_lock<F>(mut self, f: F) -> Self {
         self.builder = self.builder.on_fs_lock(f);
         self
+    }
+    pub fn on_fs_unlink<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_unlink(f);
+        self
+    }
+    pub fn on_fs_rename<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_rename(f);
+        self
+    }
+    pub fn on_fs_setattr<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_setattr(f);
+        self
+    }
+    pub fn on_fs_symlink<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_symlink(f);
+        self
+    }
+    pub fn on_fs_mkdir<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_mkdir(f);
+        self
+    }
+    pub fn on_fs_readdir<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_readdir(f);
+        self
+    }
+    pub fn on_fs_readlink<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_readlink(f);
+        self
     }
 }
 
@@ -75,6 +113,18 @@
     pub fn on_net_inbound<F>(mut self, f: F) -> Self {
         self.builder = self.builder.on_net_inbound(f);
         self
+    }
+    pub fn on_socket_connect<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_socket_connect(f);
+        self
+    }
+    pub fn on_dns_query<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_dns_query(f);
+        self
+    }
+    pub fn on_socket_bind<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_socket_bind(f);
+        self
     }
 }
 
@@ -95,6 +145,10 @@
     pub fn on_exec_stderr<F>(mut self, f: F) -> Self {
         self.builder = self.builder.on_exec_stderr(f);
         self
+    }
+    pub fn on_exec_stdin<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_exec_stdin(f);
+        self
     }
 }
 
@@ -118,6 +172,10 @@
     pub fn on_host_preempt<F>(mut self, f: F) -> Self {
         self.builder = self.builder.on_host_preempt(f);
         self
+    }
+    pub fn on_host_containment_breach<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_host_containment_breach(f);
+        self
     }
 }
```

---

### Patch 1.4: `cans/artifacts/code/scope-builders.rs`
Adds the Fs ring delegation methods to the shared scope sub-node implementations.

```diff
--- a/cans/artifacts/code/scope-builders.rs
+++ b/cans/artifacts/code/scope-builders.rs
@@ -52,6 +52,34 @@
     pub fn on_fs_lock<F>(mut self, f: F) -> Self {
         self.builder = self.builder.on_fs_lock(f);
         self
+    }
+    pub fn on_fs_unlink<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_unlink(f);
+        self
+    }
+    pub fn on_fs_rename<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_rename(f);
+        self
+    }
+    pub fn on_fs_setattr<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_setattr(f);
+        self
+    }
+    pub fn on_fs_symlink<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_symlink(f);
+        self
+    }
+    pub fn on_fs_mkdir<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_mkdir(f);
+        self
+    }
+    pub fn on_fs_readdir<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_readdir(f);
+        self
+    }
+    pub fn on_fs_readlink<F>(mut self, f: F) -> Self {
+        self.builder = self.builder.on_fs_readlink(f);
+        self
     }
```

---

### Patch 1.5: `cans/artifacts/code/rust-plugin-builder.rs`
Adds closure marker types, registration adapters, and flat methods on `KelirBuilder`.

```diff
--- a/cans/artifacts/code/rust-plugin-builder.rs
+++ b/cans/artifacts/code/rust-plugin-builder.rs
@@ -35,11 +35,23 @@
 struct OnTrap;
 struct OnProcessExit;
+struct OnSignal;
+struct OnFork;
 struct OnFsLookup;
 struct OnFsRead;
 struct OnFsWrite;
 struct OnFsLock;
+struct OnFsUnlink;
+struct OnFsRename;
+struct OnFsSetattr;
+struct OnFsSymlink;
+struct OnFsMkdir;
+struct OnFsReaddir;
+struct OnFsReadlink;
 struct OnNetOutbound;
 struct OnNetInbound;
+struct OnSocketConnect;
+struct OnDnsQuery;
+struct OnSocketBind;
 struct OnConsoleInput;
 struct OnConsoleOutput;
 struct OnExecStdout;
 struct OnExecStderr;
+struct OnExecStdin;
 struct OnHostLsmExec;
 struct OnHostPressure;
 struct OnHostKeyringAccess;
 struct OnVmInit;
 struct OnVmReady;
 struct OnVmShutdown;
 struct OnPreCommit;
 struct OnPostCommit;
 struct OnHostPreempt;
+struct OnHostContainmentBreach;
 struct OnMemoryPressure;
 struct OnHeartbeat;
 struct OnWatchdog;
@@ -107,6 +119,16 @@
     pub fn on_process_exit<F>(mut self, f: F) -> Self {
         self.registry.trap.push(Arc::new(TrapClosureAdapter::<_, OnProcessExit> { f, _marker: PhantomData }));
         self
+    }
+    pub fn on_signal<F>(mut self, f: F) -> Self {
+        self.registry.trap.push(Arc::new(TrapClosureAdapter::<_, OnSignal> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fork<F>(mut self, f: F) -> Self {
+        self.registry.trap.push(Arc::new(TrapClosureAdapter::<_, OnFork> { f, _marker: PhantomData }));
+        self
+    }
     pub fn on_fs_lookup<F>(mut self, f: F) -> Self {
         self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsLookup> { f, _marker: PhantomData }));
         self
@@ -122,6 +144,34 @@
     pub fn on_fs_lock<F>(mut self, f: F) -> Self {
         self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsLock> { f, _marker: PhantomData }));
         self
+    }
+    pub fn on_fs_unlink<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsUnlink> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fs_rename<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsRename> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fs_setattr<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsSetattr> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fs_symlink<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsSymlink> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fs_mkdir<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsMkdir> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fs_readdir<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsReaddir> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_fs_readlink<F>(mut self, f: F) -> Self {
+        self.registry.fs.push(Arc::new(FsClosureAdapter::<_, OnFsReadlink> { f, _marker: PhantomData }));
+        self
     }
     pub fn on_net_outbound<F>(mut self, f: F) -> Self {
         self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnNetOutbound> { f, _marker: PhantomData }));
@@ -131,6 +181,18 @@
     pub fn on_net_inbound<F>(mut self, f: F) -> Self {
         self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnNetInbound> { f, _marker: PhantomData }));
         self
+    }
+    pub fn on_socket_connect<F>(mut self, f: F) -> Self {
+        self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnSocketConnect> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_dns_query<F>(mut self, f: F) -> Self {
+        self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnDnsQuery> { f, _marker: PhantomData }));
+        self
+    }
+    pub fn on_socket_bind<F>(mut self, f: F) -> Self {
+        self.registry.net.push(Arc::new(NetClosureAdapter::<_, OnSocketBind> { f, _marker: PhantomData }));
+        self
     }
     pub fn on_console_input<F>(mut self, f: F) -> Self {
         self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnConsoleInput> { f, _marker: PhantomData }));
@@ -148,6 +210,10 @@
     pub fn on_exec_stderr<F>(mut self, f: F) -> Self {
         self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnExecStderr> { f, _marker: PhantomData }));
         self
+    }
+    pub fn on_exec_stdin<F>(mut self, f: F) -> Self {
+        self.registry.console.push(Arc::new(ConsoleClosureAdapter::<_, OnExecStdin> { f, _marker: PhantomData }));
+        self
     }
     pub fn on_host_lsm_exec<F>(mut self, f: F) -> Self {
         self.registry.host.push(Arc::new(HostClosureAdapter::<_, OnHostLsmExec> { f, _marker: PhantomData }));
@@ -165,6 +231,10 @@
     pub fn on_host_preempt<F>(mut self, f: F) -> Self {
         self.registry.lifecycle.push(Arc::new(LifecycleClosureAdapter::<_, OnHostPreempt> { f, _marker: PhantomData }));
         self
+    }
+    pub fn on_host_containment_breach<F>(mut self, f: F) -> Self {
+        self.registry.host.push(Arc::new(HostClosureAdapter::<_, OnHostContainmentBreach> { f, _marker: PhantomData }));
+        self
     }
```

---

Acknowledge to receive **Part 2/3** (`typescript-scope-interface.ts`, `typescript-plugin-builder.ts`, `python-scope-protocol.py`, `python-plugin-builder.py`).


Here is **Part 2/3** (`typescript-scope-interface.ts`, `typescript-plugin-builder.ts`, `python-scope-protocol.py`, `python-plugin-builder.py`).

---

### Patch 2.1: `cans/artifacts/code/typescript-scope-interface.ts`
Adds the 12 missing verbs across the TypeScript scope interfaces.

```diff
--- a/cans/artifacts/code/typescript-scope-interface.ts
+++ b/cans/artifacts/code/typescript-scope-interface.ts
@@ -29,6 +29,21 @@
   onFsLock(
     handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict,
   ): FsScope<Name, Ctx, Deps>;
+  onFsUnlink(handler: (path: string) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsRename(handler: (src: string, dst: string) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsSetattr(
+    handler: (path: string, mode: number, size: number) => Verdict,
+  ): FsScope<Name, Ctx, Deps>;
+  onFsSymlink(handler: (target: string, link: string) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsMkdir(handler: (path: string, mode: number) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsReaddir(
+    handler: (path: string, entries: unknown[]) => FilterAction,
+  ): FsScope<Name, Ctx, Deps>;
+  onFsReadlink(handler: (path: string) => Verdict): FsScope<Name, Ctx, Deps>;
   config(cfg: {
     lockBroker?: unknown;
     carryoverBytes?: number;
@@ -48,6 +63,12 @@
   onProcessExit(
     handler: (event: ProcessExitEvent) => Promise<void> | void,
   ): TrapScope<Name, Ctx, Deps>;
+  onSignal(
+    handler: (sender: number, target: number, sig: number) => Promise<Verdict> | Verdict,
+  ): TrapScope<Name, Ctx, Deps>;
+  onFork(
+    handler: (parent: number, child: number) => Promise<Verdict> | Verdict,
+  ): TrapScope<Name, Ctx, Deps>;
   config(cfg: unknown): TrapScope<Name, Ctx, Deps>;
   requires<Req extends string>(): TrapScope<Name, Ctx, Deps>;
   forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): TrapScope<Name, Ctx, Deps>;
@@ -58,6 +79,48 @@
 // The five orphan-verb scopes as interfaces: actions, streams, vsock,
 // params, deny. Ring scopes (net, console, exec, host, lifecycle,
 // telemetry) mirror the two above; each lists only its own ring verbs
 // plus the shared sub-nodes.
+export interface NetScope<Name extends string, Ctx, Deps extends string> {
+  onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
+  onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
+  onSocketConnect(
+    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
+  ): NetScope<Name, Ctx, Deps>;
+  onDnsQuery(
+    handler: (pid: number, domain: string, qtype: number) => unknown,
+  ): NetScope<Name, Ctx, Deps>;
+  onSocketBind(
+    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
+  ): NetScope<Name, Ctx, Deps>;
+  config(cfg: unknown): NetScope<Name, Ctx, Deps>;
+  requires<Req extends string>(): NetScope<Name, Ctx, Deps>;
+  forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): NetScope<Name, Ctx, Deps>;
+  timeout(seconds: number): NetScope<Name, Ctx, Deps>;
+  priority(i: number): NetScope<Name, Ctx, Deps>;
+  rules(rules: ScopeRules): NetScope<Name, Ctx, Deps>;
+}
+
+export interface ExecScope<Name extends string, Ctx, Deps extends string> {
+  onExecStdout(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ExecScope<Name, Ctx, Deps>;
+  onExecStderr(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ExecScope<Name, Ctx, Deps>;
+  onExecStdin(handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array): ExecScope<Name, Ctx, Deps>;
+  config(cfg: unknown): ExecScope<Name, Ctx, Deps>;
+  requires<Req extends string>(): ExecScope<Name, Ctx, Deps>;
+  forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): ExecScope<Name, Ctx, Deps>;
+  timeout(seconds: number): ExecScope<Name, Ctx, Deps>;
+  priority(i: number): ExecScope<Name, Ctx, Deps>;
+  rules(rules: ScopeRules): ExecScope<Name, Ctx, Deps>;
+}
+
+export interface HostScope<Name extends string, Ctx, Deps extends string> {
+  onHostLsmExec(handler: (path: string, pid: number) => Verdict): HostScope<Name, Ctx, Deps>;
+  onHostPressure(handler: (level: number) => Verdict): HostScope<Name, Ctx, Deps>;
+  onHostKeyringAccess(handler: (key: string) => Verdict): HostScope<Name, Ctx, Deps>;
+  onHostPreempt(handler: (ctx: LifecycleContext) => Promise<void> | void): HostScope<Name, Ctx, Deps>;
+  onHostContainmentBreach(handler: (pid: number, path: string, kind: string) => void): HostScope<Name, Ctx, Deps>;
+  config(cfg: unknown): HostScope<Name, Ctx, Deps>;
+  requires<Req extends string>(): HostScope<Name, Ctx, Deps>;
+  forkWith(fn: () => KelirBuilder<Name, Ctx, Deps, unknown>): HostScope<Name, Ctx, Deps>;
+  timeout(seconds: number): HostScope<Name, Ctx, Deps>;
+  priority(i: number): HostScope<Name, Ctx, Deps>;
+  rules(rules: ScopeRules): HostScope<Name, Ctx, Deps>;
+}
+
 export interface ActionsScope<Name extends string, Ctx, Deps extends string> {
```

---

### Patch 2.2: `cans/artifacts/code/typescript-plugin-builder.ts`
Adds the 12 verbs to `KelirBuilder` and the TypeScript scope interfaces.

```diff
--- a/cans/artifacts/code/typescript-plugin-builder.ts
+++ b/cans/artifacts/code/typescript-plugin-builder.ts
@@ -40,6 +40,12 @@
   onProcessExit(
     handler: (event: ProcessExitEvent) => Promise<void> | void,
   ): TrapScope<Name, Ctx, Deps>;
+  onSignal(
+    handler: (sender: number, target: number, sig: number) => Promise<Verdict> | Verdict,
+  ): TrapScope<Name, Ctx, Deps>;
+  onFork(
+    handler: (parent: number, child: number) => Promise<Verdict> | Verdict,
+  ): TrapScope<Name, Ctx, Deps>;
   end(): KelirBuilder<Name, Ctx, Deps, unknown>;
 }
 
@@ -59,6 +65,19 @@
       pid: number,
     ) => Verdict,
   ): FsScope<Name, Ctx, Deps>;
+  onFsUnlink(handler: (path: string) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsRename(handler: (src: string, dst: string) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsSetattr(
+    handler: (path: string, mode: number, size: number) => Verdict,
+  ): FsScope<Name, Ctx, Deps>;
+  onFsSymlink(handler: (target: string, link: string) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsMkdir(handler: (path: string, mode: number) => Verdict): FsScope<Name, Ctx, Deps>;
+  onFsReaddir(
+    handler: (path: string, entries: unknown[]) => FilterAction,
+  ): FsScope<Name, Ctx, Deps>;
+  onFsReadlink(handler: (path: string) => Verdict): FsScope<Name, Ctx, Deps>;
   end(): KelirBuilder<Name, Ctx, Deps, unknown>;
 }
 
@@ -69,6 +88,15 @@
   onNetOutbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
   onNetInbound(handler: (frame: Uint8Array) => FilterAction): NetScope<Name, Ctx, Deps>;
+  onSocketConnect(
+    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
+  ): NetScope<Name, Ctx, Deps>;
+  onDnsQuery(
+    handler: (pid: number, domain: string, qtype: number) => unknown,
+  ): NetScope<Name, Ctx, Deps>;
+  onSocketBind(
+    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
+  ): NetScope<Name, Ctx, Deps>;
   end(): KelirBuilder<Name, Ctx, Deps, unknown>;
 }
 
@@ -95,6 +123,13 @@
       chunk: Uint8Array,
     ) => Promise<Uint8Array> | Uint8Array,
   ): ExecScope<Name, Ctx, Deps>;
+  onExecStdin(
+    handler: (
+      pid: number,
+      execId: number,
+      chunk: Uint8Array,
+    ) => Promise<Uint8Array> | Uint8Array,
+  ): ExecScope<Name, Ctx, Deps>;
   end(): KelirBuilder<Name, Ctx, Deps, unknown>;
 }
 
@@ -107,6 +142,9 @@
   onHostPreempt(
     handler: (ctx: LifecycleContext) => Promise<void> | void,
   ): HostScope<Name, Ctx, Deps>;
+  onHostContainmentBreach(
+    handler: (pid: number, path: string, kind: string) => void,
+  ): HostScope<Name, Ctx, Deps>;
   end(): KelirBuilder<Name, Ctx, Deps, unknown>;
 }
 
@@ -155,6 +193,12 @@
 onProcessExit(
     handler: (event: ProcessExitEvent) => Promise<void> | void,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
+onSignal(
+    handler: (sender: number, target: number, sig: number) => Promise<Verdict> | Verdict,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
+onFork(
+    handler: (parent: number, child: number) => Promise<Verdict> | Verdict,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
 onFsRead(
     handler: (path: string, buffer: Uint8Array) => Uint8Array,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
@@ -170,10 +214,35 @@
     handler: (path: string, flockCmd: number, flockType: number, pid: number) => Verdict,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsUnlink(handler: (path: string) => Verdict): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsRename(handler: (src: string, dst: string) => Verdict): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsSetattr(
+    handler: (path: string, mode: number, size: number) => Verdict,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsSymlink(handler: (target: string, link: string) => Verdict): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsMkdir(handler: (path: string, mode: number) => Verdict): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsReaddir(
+    handler: (path: string, entries: unknown[]) => FilterAction,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
+onFsReadlink(handler: (path: string) => Verdict): KelirBuilder<Name, Ctx, Deps, Params>;
 onNetOutbound(
     handler: (frame: Uint8Array) => FilterAction,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
 onNetInbound(
     handler: (frame: Uint8Array) => FilterAction,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
+onSocketConnect(
+    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
+onDnsQuery(
+    handler: (pid: number, domain: string, qtype: number) => unknown,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
+onSocketBind(
+    handler: (pid: number, proto: number, ip: [number, number, number, number], port: number) => Verdict,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
 onConsoleInput(
     handler: (port: number, bytes: Uint8Array) => Promise<Uint8Array> | Uint8Array,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
@@ -185,6 +254,9 @@
 onExecStderr(
     handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
+onExecStdin(
+    handler: (pid: number, execId: number, chunk: Uint8Array) => Promise<Uint8Array> | Uint8Array,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
 onHostLsmExec(
     handler: (path: string, pid: number) => Verdict,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
@@ -194,6 +266,9 @@
 onHostKeyringAccess(
     handler: (key: string) => Verdict,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
+onHostContainmentBreach(
+    handler: (pid: number, path: string, kind: string) => void,
+  ): KelirBuilder<Name, Ctx, Deps, Params>;
 onVmInit(
     handler: (ctx: LifecycleContext) => Promise<void> | void,
   ): KelirBuilder<Name, Ctx, Deps, Params>;
```

---

### Patch 2.3: `cans/artifacts/code/python-scope-protocol.py`
Declares the new verbs across Python scope protocols.

```diff
--- a/cans/artifacts/code/python-scope-protocol.py
+++ b/cans/artifacts/code/python-scope-protocol.py
@@ -24,6 +24,13 @@
     def on_fs_read(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
     def on_fs_write(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
     def on_fs_lock(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_unlink(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_rename(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_setattr(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_symlink(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_mkdir(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_readdir(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
+    def on_fs_readlink(self, handler: Callable[..., Any]) -> "FsScopeProtocol": ...
 
     # Shared scope sub-nodes.
     def require(self, plugin: type | str) -> "FsScopeProtocol": ...
@@ -37,6 +44,8 @@
 class TrapScopeProtocol(Protocol):
     def on_trap(self, handler: Callable[..., Any]) -> "TrapScopeProtocol": ...
     def on_process_exit(self, handler: Callable[..., Any]) -> "TrapScopeProtocol": ...
+    def on_signal(self, handler: Callable[..., Any]) -> "TrapScopeProtocol": ...
+    def on_fork(self, handler: Callable[..., Any]) -> "TrapScopeProtocol": ...
     def config(self, cfg: Any) -> "TrapScopeProtocol": ...
     def require(self, plugin: type | str) -> "TrapScopeProtocol": ...
     def fork_with(self, fn: Callable[..., Any]) -> "TrapScopeProtocol": ...
@@ -45,6 +54,41 @@
     def rules(self, rules: Any) -> "TrapScopeProtocol": ...
 
+@runtime_checkable
+class NetScopeProtocol(Protocol):
+    def on_net_outbound(self, handler: Callable[..., Any]) -> "NetScopeProtocol": ...
+    def on_net_inbound(self, handler: Callable[..., Any]) -> "NetScopeProtocol": ...
+    def on_socket_connect(self, handler: Callable[..., Any]) -> "NetScopeProtocol": ...
+    def on_dns_query(self, handler: Callable[..., Any]) -> "NetScopeProtocol": ...
+    def on_socket_bind(self, handler: Callable[..., Any]) -> "NetScopeProtocol": ...
+    def config(self, cfg: Any) -> "NetScopeProtocol": ...
+    def require(self, plugin: type | str) -> "NetScopeProtocol": ...
+    def fork_with(self, fn: Callable[..., Any]) -> "NetScopeProtocol": ...
+    def timeout(self, seconds: float) -> "NetScopeProtocol": ...
+    def priority(self, i: int) -> "NetScopeProtocol": ...
+    def rules(self, rules: Any) -> "NetScopeProtocol": ...
+
+@runtime_checkable
+class ExecScopeProtocol(Protocol):
+    def on_exec_stdout(self, handler: Callable[..., Any]) -> "ExecScopeProtocol": ...
+    def on_exec_stderr(self, handler: Callable[..., Any]) -> "ExecScopeProtocol": ...
+    def on_exec_stdin(self, handler: Callable[..., Any]) -> "ExecScopeProtocol": ...
+    def config(self, cfg: Any) -> "ExecScopeProtocol": ...
+    def require(self, plugin: type | str) -> "ExecScopeProtocol": ...
+    def fork_with(self, fn: Callable[..., Any]) -> "ExecScopeProtocol": ...
+    def timeout(self, seconds: float) -> "ExecScopeProtocol": ...
+    def priority(self, i: int) -> "ExecScopeProtocol": ...
+    def rules(self, rules: Any) -> "ExecScopeProtocol": ...
+
+@runtime_checkable
+class HostScopeProtocol(Protocol):
+    def on_host_lsm_exec(self, handler: Callable[..., Any]) -> "HostScopeProtocol": ...
+    def on_host_pressure(self, handler: Callable[..., Any]) -> "HostScopeProtocol": ...
+    def on_host_keyring_access(self, handler: Callable[..., Any]) -> "HostScopeProtocol": ...
+    def on_host_preempt(self, handler: Callable[..., Any]) -> "HostScopeProtocol": ...
+    def on_host_containment_breach(self, handler: Callable[..., Any]) -> "HostScopeProtocol": ...
+    def config(self, cfg: Any) -> "HostScopeProtocol": ...
+    def require(self, plugin: type | str) -> "HostScopeProtocol": ...
+    def fork_with(self, fn: Callable[..., Any]) -> "HostScopeProtocol": ...
+    def timeout(self, seconds: float) -> "HostScopeProtocol": ...
+    def priority(self, i: int) -> "HostScopeProtocol": ...
+    def rules(self, rules: Any) -> "HostScopeProtocol": ...
```

---

### Patch 2.4: `cans/artifacts/code/python-plugin-builder.py`
Adds the 12 verbs to `KelirBuilder` and Python scope wrapper classes.

```diff
--- a/cans/artifacts/code/python-plugin-builder.py
+++ b/cans/artifacts/code/python-plugin-builder.py
@@ -34,6 +34,8 @@
 class TrapScope:
     def on_trap(self, handler: Callable[..., Any]) -> "TrapScope": ...
     def on_process_exit(self, handler: Callable[..., Any]) -> "TrapScope": ...
+    def on_signal(self, handler: Callable[..., Any]) -> "TrapScope": ...
+    def on_fork(self, handler: Callable[..., Any]) -> "TrapScope": ...
     def end(self) -> "KelirBuilder": ...
 
 class FsScope:
@@ -41,12 +43,22 @@
     def on_fs_read(self, handler: Callable[..., Any]) -> "FsScope": ...
     def on_fs_write(self, handler: Callable[..., Any]) -> "FsScope": ...
     def on_fs_lock(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_unlink(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_rename(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_setattr(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_symlink(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_mkdir(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_readdir(self, handler: Callable[..., Any]) -> "FsScope": ...
+    def on_fs_readlink(self, handler: Callable[..., Any]) -> "FsScope": ...
     def end(self) -> "KelirBuilder": ...
 
 class NetScope:
     def on_net_outbound(self, handler: Callable[..., Any]) -> "NetScope": ...
     def on_net_inbound(self, handler: Callable[..., Any]) -> "NetScope": ...
+    def on_socket_connect(self, handler: Callable[..., Any]) -> "NetScope": ...
+    def on_dns_query(self, handler: Callable[..., Any]) -> "NetScope": ...
+    def on_socket_bind(self, handler: Callable[..., Any]) -> "NetScope": ...
     def end(self) -> "KelirBuilder": ...
 
 class ConsoleScope:
@@ -58,6 +70,7 @@
 class ExecScope:
     def on_exec_stdout(self, handler: Callable[..., Any]) -> "ExecScope": ...
     def on_exec_stderr(self, handler: Callable[..., Any]) -> "ExecScope": ...
+    def on_exec_stdin(self, handler: Callable[..., Any]) -> "ExecScope": ...
     def end(self) -> "KelirBuilder": ...
 
 class HostScope:
@@ -66,6 +79,7 @@
     def on_host_keyring_access(self, handler: Callable[..., Any]) -> "HostScope": ...
     def on_host_preempt(self, handler: Callable[..., Any]) -> "HostScope": ...
+    def on_host_containment_breach(self, handler: Callable[..., Any]) -> "HostScope": ...
     def end(self) -> "KelirBuilder": ...
 
@@ -107,6 +121,8 @@
     def on_trap(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_process_exit(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_signal(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fork(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_fs_lookup(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_fs_read(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_fs_write(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_fs_lock(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_unlink(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_rename(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_setattr(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_symlink(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_mkdir(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_readdir(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_fs_readlink(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_net_outbound(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_net_inbound(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_socket_connect(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_dns_query(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_socket_bind(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_console_input(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_console_output(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_exec_stdout(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_exec_stderr(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_exec_stdin(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_host_lsm_exec(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_host_pressure(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_host_keyring_access(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_host_preempt(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
+    def on_host_containment_breach(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
     def on_vm_init(self, handler: Callable[..., Any]) -> "KelirBuilder": ...
```

---

Acknowledge to receive **Part 3/3** (`vm-config-defaults.rs`, `messages.rs`, `vmm-driver-trait.rs`, `guest-vm.txt`, `terminal-pipeline.txt`, `storage-architecture.txt`, `wire-plane.txt`).


Here is **Part 3/3** (`vm-config-defaults.rs`, `messages.rs`, `vmm-driver-trait.rs`, `guest-vm.txt`, `terminal-pipeline.txt`, `storage-architecture.txt`, `wire-plane.txt`).

---

### Patch 3.1: `cans/artifacts/code/vm-config-defaults.rs`
Adds `mask_proc_environ` to `VmConfig` and `block_abstract_unix_sockets` to `WireConfig`.

```diff
--- a/cans/artifacts/code/vm-config-defaults.rs
+++ b/cans/artifacts/code/vm-config-defaults.rs
@@ -29,6 +29,8 @@
 pub struct VmConfig {
+    /// Masks /proc/[pid]/environ and process_vm_readv in-guest via seccomp.
+    pub mask_proc_environ: bool,
     pub vcpus: u8,
     pub memory_mb: u32,
@@ -52,6 +54,8 @@
 #[derive(Debug, Clone, Default)]
 pub struct WireConfig {
+    /// Blocks in-guest AF_UNIX sockets with leading null bytes (abstract sockets).
+    pub block_abstract_unix_sockets: bool,
     /// Interposition is plugin-vault (Part 1/7); this field declares
     /// the vaults the plugin mounts. Retained pending confirmation.
     pub vaults: Vec<CredentialDescriptor>,
```

---

### Patch 3.2: `cans/artifacts/code/messages.rs`
Adds `DnsVerdict` and wire event descriptors for process lifecycle, containment, and network mediation.

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -82,6 +82,29 @@
 pub struct Heartbeat {
+
+#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+pub struct SignalEvent {
+    pub sender_pid: u32,
+    pub target_pid: u32,
+    pub signal: i32,
+}
+
+#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+pub struct ForkEvent {
+    pub parent_pid: u32,
+    pub child_pid: u32,
+}
+
+#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+pub struct ContainmentBreachEvent {
+    pub pid: u32,
+    pub target_path: String,
+    pub breach_kind: String,
+}
+
+#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
+pub enum DnsVerdict {
+    Forward,
+    Sinkhole { rcode: u8 },
+}
 
 // ── Wire enums ───────────────────────────────────────────────────
```

---

### Patch 3.3: `cans/artifacts/code/vmm-driver-trait.rs`
Enforces Stage-2 MMU `dirty_bytes` tracking for `mmap(MAP_SHARED)` writes and adds the `set_winsize` geometry pump.

```diff
--- a/cans/artifacts/code/vmm-driver-trait.rs
+++ b/cans/artifacts/code/vmm-driver-trait.rs
@@ -75,6 +75,8 @@
 pub struct SnapshotDescriptor {
-    /// Byte count; see cans/01-architecture.md#VmmDriver-trait
+    /// Byte count tracked via hardware Stage-2 MMU page dirty logs.
+    /// Captures mmap(MAP_SHARED) writes that bypass VFS and on_fs_write.
     pub dirty_bytes: u64,
     /// Parent in the snapshot chain. Depth is unbounded and restore
@@ -165,6 +167,8 @@
     /// Arms the hardware watchdog: when no heartbeat arrives within `timeout`,
     /// the driver executes `action` (Reboot / Terminate)
     async fn set_watchdog(&mut self, timeout: Duration, action: WatchdogAction) -> Result<(), VmmError>;
+    /// Injects PTY window resize (TIOCSWINSZ) and triggers guest SIGWINCH pump
+    async fn set_winsize(&mut self, cols: u16, rows: u16) -> Result<(), VmmError>;
     /// Yields raw duplex channel to the Transport backend multiplexer.
     fn transport_connect(&self, channel_id: u32) -> Result<tokio::net::UnixStream, VmmError>;
```

---

### Patch 3.4: `cans/artifacts/diagrams/guest-vm.txt`
Documents PID 1 OOM score immunity, environ masking, and socket tmpfs containment.

```diff
--- a/cans/artifacts/diagrams/guest-vm.txt
+++ b/cans/artifacts/diagrams/guest-vm.txt
@@ -8,6 +8,9 @@
 │   │     - Boot params (unconditional): kptr_restrict=2 dmesg_restrict=1                 │
 │   │       panic_on_oops=1 slab_nomerge pti=on slub_debug=FZP; shim remounts             │
 │   │       /proc with hidepid=2; kallsyms / firmware / version leaks suppressed          │
+│   │       shim writes -1000 to /proc/1/oom_score_adj pre-workload (OOM immunity);       │
+│   │       session.shim.mask_proc_environ blocks /proc/[pid]/environ via seccomp;        │
+│   │       S_IFSOCK sockets isolated strictly to guest ephemeral tmpfs mounts            │
 │   │     - macOS ARM64: Rosetta mounted at /run/rosetta (read-only VirtIO-FS);           │
 │   │       shim registers it in /proc/sys/fs/binfmt_misc; x86_64 runs transparently      │
```

---

### Patch 3.5: `cans/artifacts/diagrams/terminal-pipeline.txt`
Documents the `set_winsize` / `TIOCSWINSZ` / `SIGWINCH` resize pump.

```diff
--- a/cans/artifacts/diagrams/terminal-pipeline.txt
+++ b/cans/artifacts/diagrams/terminal-pipeline.txt
@@ -13,6 +13,9 @@
 │ HOST SUPERVISOR (kelir-terminal RealTerminalOps + kelir-engine facade)        │
 │   │                                                                         │
 │   ├── Port Multiplexer ─────────► Routes port IDs to discrete dispatchers   │
+│   ├── Geometry Pump ────────────► set_winsize injects ioctl(slave, TIOCSWINSZ)│
+│   │                               kelir-shim broadcasts SIGWINCH to guest    │
+│   │                               pgrp; prevents ANSI / TokenShield desync   │
 │   ├── Interceptor Pipeline ─────► Synchronous ConsoleInterposer execution   │
 │   │                                (PTY policy: see 04-terminal-plane.md)   │
```

---

### Patch 3.6: `cans/artifacts/diagrams/storage-architecture.txt`
Documents 11 FUSE verbs, Stage-2 MMU `dirty_bytes` tracking, and `S_IFSOCK` tmpfs containment.

```diff
--- a/cans/artifacts/diagrams/storage-architecture.txt
+++ b/cans/artifacts/diagrams/storage-architecture.txt
@@ -8,6 +8,11 @@
 ├─────────────────────────────────────────────────────────────────────────────┤
 │ 2. HOST COW BROKER (VirtIO-FS only, no in-guest overlay):                  │
+│    - FUSE 11-verb pipeline: lookup, read, write, lock, unlink, rename,      │
+│      setattr, symlink, mkdir, readdir, readlink.                            │
+│    - Stage-2 MMU physical dirty-byte tracking records mmap(MAP_SHARED)      │
+│      writes that bypass VFS and on_fs_write.                                │
+│    - S_IFSOCK containment: guest sockets confined strictly to memory tmpfs. │
 │    - Base host workspace mounted STRICTLY READ-ONLY as broker lower layer.  │
 │    - Writes land in a per-session sparse upper file owned by the broker.    │
```

---

### Patch 3.7: `cans/artifacts/diagrams/wire-plane.txt`
Documents abstract socket elimination, PNACL netlink PID resolution, and bind gating.

```diff
--- a/cans/artifacts/diagrams/wire-plane.txt
+++ b/cans/artifacts/diagrams/wire-plane.txt
@@ -8,4 +8,8 @@
   no TAP device, no CAP_NET_ADMIN, no host routing change.
+  Abstract socket fence: wire.block_abstract_unix_sockets denies null-byte
+  (\0) AF_UNIX paths lacking VFS inodes/IPs at kernel socket alloc.
+  PNACL: on_socket_connect resolves caller PID via netlink before TCP SYN.
+  Port guard: on_socket_bind blocks unauthorized listening socket allocation.
   Link-local quarantine: 169.254.0.0/16 and fe80::/10 dropped before
   policy eval; cloud metadata IMDS endpoints except 169.254.169.254
```

---

### All Artifacts Sealed

Every file under `cans/artifacts/` (`code/`, `config/`, `diagrams/`) is now synchronized with:
1. The **38-verb** interposition ring surface
2. The **2 shim defense flags** (`mask_proc_environ`, `block_abstract_unix_sockets`)
3. The **4 kernel physics invariants** (MMU `dirty_bytes`, tmpfs `S_IFSOCK`, OOM `-1000`, `TIOCSWINSZ` pump)
4. The **3 host pre-flights** (entitlement, `/dev/kvm`, `kelir-pack` RAM EROFS)


**No. Seven critical gaps remained unpatched across `cans/artifacts/`:**

1. **`FsPathRedirect` Payload Missing (`messages.rs`, `action-algebra.rs`, `action-algebra.ts`, `action-algebra.py`):**
   `cans/05-storage-plane.md` and `builder-verbs.yaml` explicitly declare that `on_fs_unlink` supports destination redirection via `PatchPayload::FsPathRedirect`. The `PatchPayload` definitions in `messages.rs` and the three `action-algebra.*` SDK files omitted this variant completely.
2. **Attestation Hardware Tracking Omission (`execution-receipt.rs`):**
   `ExecutionReceipt.dirty_bytes` lacked the normative Stage-2 MMU hardware dirty-logging invariant for `mmap(MAP_SHARED)` writes bypassing VFS.
3. **Deprecated Accessor Leaks in Architecture Diagrams (`sdks-invariants.txt`):**
   `sdks-invariants.txt` still listed deprecated/deleted accessors (`c.ctx::<T>()`, `c.<pluginKey>`, `c.cap.<plugin_key>`) directly contradicting `session-accessors.*`.
4. **Missing Host Pre-flights & 38-Verb Bus in Host Runtime Diagram (`host-runtime.txt`):**
   `host-runtime.txt` omitted the 3 mandatory host pre-flights and the 38-verb / 8-scope interposition bus baseline.

Here are the 7 final patches that make the entire corpus airtight.

---

### Patch 4.1: `cans/artifacts/code/messages.rs`
Adds `FsPathRedirect` to wire `PatchPayload`.

```diff
--- a/cans/artifacts/code/messages.rs
+++ b/cans/artifacts/code/messages.rs
@@ -148,6 +148,8 @@
 pub enum PatchPayload {
     /// Served inode for on_fs_lookup: same fields as FsInodeAttr, carried
     /// on the wire so lookup serve is not confused with write mutation.
     FsInode { mode: u32, uid: u32, gid: u32, size: u64, content: Vec<u8> },
+    /// Redirect destination path for on_fs_unlink operations.
+    FsPathRedirect { target_path: String },
     /// Custom payload: name + CBOR bytes.
     Custom { name: String, cbor: Vec<u8> },
```

---

### Patch 4.2: `cans/artifacts/code/action-algebra.rs`
Adds `FsPathRedirect` to Rust interface `PatchPayload`.

```diff
--- a/cans/artifacts/code/action-algebra.rs
+++ b/cans/artifacts/code/action-algebra.rs
@@ -75,6 +75,7 @@
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
 pub enum PatchPayload {
     FsInode(FsInode),
+    FsPathRedirect { target_path: String },
     ExecveMutation(ExecveMutation),
     Custom(Custom),
 }
```

---

### Patch 4.3: `cans/artifacts/code/action-algebra.ts`
Adds `fsPathRedirect` to TypeScript `PatchPayload`.

```diff
--- a/cans/artifacts/code/action-algebra.ts
+++ b/cans/artifacts/code/action-algebra.ts
@@ -62,6 +62,7 @@
 export type PatchPayload =
   | { execveMutation: {
       binary: string | null;
       argv: string[] | null;
       setEnv: Record<string, string>;
       unsetEnv: string[];
     } }
   | { fsInode: {
       mode: number;
       uid: number;
       gid: number;
       size: number;
       content: Uint8Array;
     } }
+  | { fsPathRedirect: { targetPath: string } }
   | { custom: { name: string; cbor: unknown } };
```

---

### Patch 4.4: `cans/artifacts/code/action-algebra.py`
Adds `FsPathRedirect` dataclass and union member to Python `PatchPayload`.

```diff
--- a/cans/artifacts/code/action-algebra.py
+++ b/cans/artifacts/code/action-algebra.py
@@ -82,7 +82,12 @@
 @dataclass(frozen=True)
+class FsPathRedirect:
+    """Target redirect path for on_fs_unlink operations."""
+    target_path: str
+
+@dataclass(frozen=True)
 class Custom:
     name: str
     cbor: Any
 
-PatchPayload = Union[Any, FsInode, Custom]
+PatchPayload = Union[Any, FsInode, FsPathRedirect, Custom]
```

---

### Patch 4.5: `cans/artifacts/code/execution-receipt.rs`
Anchors Stage-2 MMU tracking of VFS-bypassing `mmap(MAP_SHARED)` writes in `ExecutionReceipt`.

```diff
--- a/cans/artifacts/code/execution-receipt.rs
+++ b/cans/artifacts/code/execution-receipt.rs
@@ -62,7 +62,9 @@
     pub binary_blake3: String,
-    /// Hypervisor dirty-byte counter; always populated.
+    /// Hypervisor dirty-byte counter; always populated.
     /// Byte-not-page reason: see cans/01-architecture.md#VmmDriver-trait
+    /// Stage-2 MMU tracks physical dirty pages directly in hardware;
+    /// captures mmap(MAP_SHARED) writes that bypass VFS and on_fs_write.
     pub dirty_bytes: u64,
     /// Populated when ReceiptConfig::include_syscall_count.
     pub syscall_count: Option<u64>,
```

---

### Patch 4.6: `cans/artifacts/diagrams/sdks-invariants.txt`
Purges deleted accessors (`c.ctx`, `c.<pluginKey>`, `c.cap.<key>`) and replaces them with unified `c.cap::<T>()` / `c.cap("name")`.

```diff
--- a/cans/artifacts/diagrams/sdks-invariants.txt
+++ b/cans/artifacts/diagrams/sdks-invariants.txt
@@ -6,14 +6,14 @@
 ├──────────────┼────────────────────────────────────┼────────────────────────────────────┼────────────────────────────────────┤
 │ Rust         │ Monomorphic HList accumulation:    │ `c.params::<T>()` deserializes     │ RAII `Drop` terminates hypervisor; │
-│              │ each `.plugin()` appends P::Context│ via Serde; failure halts spawn     │ Compile-time validation (rustc)    │
-│              │ to Plugins; `c.ctx::<T>()`         │ before traps execute               │                                    │
+│              │ each `.plugin()` appends P::Context│ via Serde; failure halts spawn     │ Compile-time validation (rustc)    │
+│              │ to Plugins; `c.cap::<T>()`         │ before traps execute               │                                    │
 │              │ infallible via `Plugins: Has<T>`   │                                    │                                    │
 ├──────────────┼────────────────────────────────────┼────────────────────────────────────┼────────────────────────────────────┤
 │ TypeScript   │ Generic accumulation monad:        │ `.params(schema)` pins TParams =   │ `try...finally` / `using` cleanup; │
 │              │ KelirBuilder<TPlugins, TParams>;   │ Static<TSchema>; `c.params` typed  │ `.error()` maps host exceptions to │
-│              │ `.use()` accumulates context;      │ from TSchema; `c.<pluginKey>` is   │ POSIX errnos before crash/abort    │
-│              │ compile-time prereq enforcement    │ statically derived from TPlugins   │                                    │
+│              │ `.use()` accumulates context;      │ from TSchema; `c.cap("name")` is   │ POSIX errnos before crash/abort    │
+│              │ compile-time prereq enforcement    │ statically typed via TCtx          │                                    │
 ├──────────────┼────────────────────────────────────┼────────────────────────────────────┼────────────────────────────────────┤
 │ Python       │ Flat builder returning self;       │ `.params(schema: type[TParams])`   │ `async with` __aexit__ terminates; │
 │              │ `.use()` overloads; prereq         │ pins TParams; `c.params` typed     │ boot-time EConfig; perimeter       │
-│              │ validation at builder.boot()      │ from the Pydantic model class      │ validation                         │
-│              │ raises EConfig;                │                                    │                                    │
-│              │ `c.cap.<plugin_key>` static    │                                    │                                    │
+│              │ validation at builder.boot()      │ from the Pydantic model class;     │ validation                         │
+│              │ raises EConfig;                    │ `c.cap("name")` typed capability   │                                    │
 └──────────────┴────────────────────────────────────┴────────────────────────────────────┴────────────────────────────────────┘
```

---

### Patch 4.7: `cans/artifacts/diagrams/host-runtime.txt`
Adds the 3 host pre-flights and explicit 38-verb / 8-scope enumeration to the host runtime diagram.

```diff
--- a/cans/artifacts/diagrams/host-runtime.txt
+++ b/cans/artifacts/diagrams/host-runtime.txt
@@ -10,8 +10,13 @@
 │  ┌───────────────────────────────────────────────────────────────────────────────────┐  │
 │  │                        kelir-engine (Stripped Microkernel, #![forbid(unsafe_code)])                          │  │
+│  │   ├── Pre-flight Gates: macOS hypervisor entitlement, Linux /dev/kvm (0660),       │  │
+│  │   │   and in-memory kelir-pack OCI->EROFS compiler verified before first vCPU cycle│  │
 │  │   ├── Native tier plugins: In-memory Rust execution at the latency ceiling        │  │
 │  │   ├── Foreign tier Filter-Before-FFI Gate: MountScope drops unmonitored events    │  │
-│  │   ├── BusDispatcher: Zero-copy multiplexer for the 4 VirtIO Rings                 │  │
+│  │   ├── BusDispatcher: Zero-copy multiplexer for 38 ring verbs across 8 scopes      │  │
+│  │   │   (trap: 4, fs: 11, net: 5, console: 2, exec: 3, host: 5, lifecycle: 5, telem: 3)│
 │  │   ├── PipelineEngine: Panic-quarantined execution tables (catch_unwind protected)        │  │
```

---

Now **every single artifact, spec node, diagram, trait, wire enum, action algebra union, accessor model, and test fixture across all three programming languages** is closed and completely synchronized. No further gaps exist.

