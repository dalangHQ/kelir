// see: cans/11-protocol.md#Wire-framing
use std::fmt::Write as _;

use kelir_interface::codec_protocol::{
    FLAG_CLOSE, FLAG_DATA, FLAG_OPEN, FLAG_RST, FLAG_WINDOW_UPDATE, MAX_CONTROL_PAYLOAD,
    MAX_FRAME_PAYLOAD, MAX_STREAM_ID,
};
use kelir_interface::protocol_wire::{CHANNELS, EXEC_STREAMS, MSG_TYPES, STREAM_KINDS};

use crate::naming_codegen::screaming;

const WIRE_H_SEE: &str = "// see: cans/11-protocol.md#Wire-framing
// see: cans/artifacts/code/wire-protocol.rs#MuxFrameHeader
";

// see: cans/11-protocol.md#Wire-framing
pub fn emit_wire_h() -> String {
    let mut out = String::new();
    out.push_str(WIRE_H_SEE);
    out.push_str("#ifndef KELIR_WIRE_H\n#define KELIR_WIRE_H\n\n");
    for (name, value) in CHANNELS {
        let _ = writeln!(
            out,
            "#define KELIR_CHANNEL_{} {value:#06x}",
            screaming(name)
        );
    }
    out.push('\n');
    for (name, value) in MSG_TYPES {
        let _ = writeln!(
            out,
            "#define KELIR_MSG_TYPE_{} {value:#06x}",
            screaming(name)
        );
    }
    out.push('\n');
    for (name, value) in STREAM_KINDS {
        let _ = writeln!(
            out,
            "#define KELIR_STREAM_KIND_{} {value:#06x}",
            screaming(name)
        );
    }
    out.push('\n');
    for (name, value) in EXEC_STREAMS {
        let _ = writeln!(out, "#define KELIR_{name} {value:#04x}");
    }
    out.push('\n');
    for (name, value) in [
        ("FLAG_OPEN", FLAG_OPEN),
        ("FLAG_DATA", FLAG_DATA),
        ("FLAG_CLOSE", FLAG_CLOSE),
        ("FLAG_RST", FLAG_RST),
        ("FLAG_WINDOW_UPDATE", FLAG_WINDOW_UPDATE),
    ] {
        let _ = writeln!(out, "#define KELIR_{name} {value:#04x}");
    }
    out.push('\n');
    let _ = writeln!(out, "#define KELIR_MAX_STREAM_ID {MAX_STREAM_ID:#010x}");
    let _ = writeln!(out, "#define KELIR_MAX_FRAME_PAYLOAD {MAX_FRAME_PAYLOAD}");
    let _ = writeln!(
        out,
        "#define KELIR_MAX_CONTROL_PAYLOAD {MAX_CONTROL_PAYLOAD}"
    );
    out.push_str("\n#endif\n");
    out
}
