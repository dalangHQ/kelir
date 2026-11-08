# see: cans/11-protocol.md#Wire-framing
# see: cans/11-protocol.md#Control-channel
# see: cans/11-protocol.md#Binary-exec-channel
from enum import IntEnum

class Channel(IntEnum):
    CONTROL = 0x0000
    STREAM_MUX = 0x0001

class MsgType(IntEnum):
    TRAP = 0x0001
    CHANNEL_PAYLOAD = 0x0002
    VM_LIFECYCLE_CONTROL = 0x0003
    SYSTEM_TELEMETRY = 0x0004
    SIGNAL = 0x0005

class StreamKind(IntEnum):
    EXEC = 0x0001
    PTY_CONTROL = 0x0002
    VFS = 0x0003
    BULK = 0x0004
    L4_FORWARD = 0x0005
    TERMINAL_CONSOLE = 0x0006
    PROFILE_SYNC = 0x0010
    DB_SQLITE = 0x0011
    MCP_CONSOLE = 0x0012

class ExecStream(IntEnum):
    STDIN = 0x00
    STDOUT = 0x01
    STDERR = 0x02
    CONTROL = 0x03

FLAG_OPEN = 0x01
FLAG_DATA = 0x02
FLAG_CLOSE = 0x04
FLAG_RST = 0x08
FLAG_WINDOW_UPDATE = 0x10
MAX_STREAM_ID = 0xffffff
MAX_FRAME_PAYLOAD = 65536
MAX_CONTROL_PAYLOAD = 4096
