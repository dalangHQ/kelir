// see: cans/03-sdk-architecture.md#Native-Engine-Planes-on-Session
// TODO (L3) [bindings_ts]: implement async NativeVmHandle / NativeSessionHandle bodies across the addon; slop: handle methods never cross the addon, calls throw instead of dispatching.
// L3-GAP: [bindings_ts]: async handle bodies for NativeVmHandle / NativeSessionHandle (checkpoint, rollback, suspend, resume, fork, spawn, exec_in_guest, stop, terminate) relocate from handles.flow.rs; slop: handle methods return fabricated records without crossing the addon
