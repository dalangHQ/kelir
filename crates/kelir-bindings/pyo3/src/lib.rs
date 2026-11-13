// see: cans/03-sdk-architecture.md#Python-SDK
// see: cans/01-architecture.md#Foreign-runtime-embedding
// see: cans/02-plugin-model.md#Foreign-plugin-proxy
#![forbid(unsafe_code)]

#[path = "builder.flow.rs"]
pub mod builder;
#[path = "fusion_plugin.flow.rs"]
pub mod fusion_plugin;
#[path = "handles.flow.rs"]
pub mod handles;
#[path = "handles.run.rs"]
pub mod handles_run;
pub mod storage_bridge;

pub use builder::PyKelirBuilder;
pub(crate) use handles::envelope_error;
pub use handles::{
    PyCheckpointHandle, PyHostHandle, PySessionHandle, PyStepHandle, PyStorageHandle,
    PyTerminalHandle, PyVmHandle, PyVsockHandle, PyWireHandle,
};

use handles::relay_socket_path;
use pyo3::prelude::*;

#[pymodule]
fn _kelir(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(relay_socket_path, m)?)?;
    m.add_class::<PyKelirBuilder>()?;
    m.add_class::<PySessionHandle>()?;
    m.add_class::<PyVmHandle>()?;
    m.add_class::<PyTerminalHandle>()?;
    m.add_class::<PyStorageHandle>()?;
    m.add_class::<PyWireHandle>()?;
    m.add_class::<PyCheckpointHandle>()?;
    m.add_class::<PyHostHandle>()?;
    m.add_class::<PyVsockHandle>()?;
    m.add_class::<PyStepHandle>()?;
    Ok(())
}
