// see: cans/11-protocol.md#Wire-framing
// see: cans/11-protocol.md#Control-channel
// see: cans/11-protocol.md#Binary-exec-channel

export const CHANNEL_CONTROL = 0x0000;
export const CHANNEL_STREAM_MUX = 0x0001;

export const MSG_TYPE_TRAP = 0x0001;
export const MSG_TYPE_CHANNEL_PAYLOAD = 0x0002;
export const MSG_TYPE_VM_LIFECYCLE_CONTROL = 0x0003;
export const MSG_TYPE_SYSTEM_TELEMETRY = 0x0004;
export const MSG_TYPE_SIGNAL = 0x0005;

export const STREAM_KIND_EXEC = 0x0001;
export const STREAM_KIND_PTY_CONTROL = 0x0002;
export const STREAM_KIND_VFS = 0x0003;
export const STREAM_KIND_BULK = 0x0004;
export const STREAM_KIND_L4_FORWARD = 0x0005;
export const STREAM_KIND_TERMINAL_CONSOLE = 0x0006;
export const STREAM_KIND_PROFILE_SYNC = 0x0010;
export const STREAM_KIND_DB_SQLITE = 0x0011;
export const STREAM_KIND_MCP_CONSOLE = 0x0012;

export const EXEC_STREAM_STDIN = 0x00;
export const EXEC_STREAM_STDOUT = 0x01;
export const EXEC_STREAM_STDERR = 0x02;
export const EXEC_STREAM_CONTROL = 0x03;

export const FLAG_OPEN = 0x01;
export const FLAG_DATA = 0x02;
export const FLAG_CLOSE = 0x04;
export const FLAG_RST = 0x08;
export const FLAG_WINDOW_UPDATE = 0x10;

export const MAX_STREAM_ID = 0xffffff;
export const MAX_FRAME_PAYLOAD = 65536;
export const MAX_CONTROL_PAYLOAD = 4096;
