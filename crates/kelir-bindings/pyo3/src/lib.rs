// see: cans/03-sdk-architecture.md#Python-SDK
#![forbid(unsafe_code)]
use pyo3::prelude::*;

#[pyclass]
pub struct PySessionHandle {}

#[pyclass]
pub struct PyKelirBuilder {}

#[pymethods]
impl PyKelirBuilder {
    #[new]
    fn new() -> Self {
        Self {}
    }

    fn boot(&self) -> PyResult<PySessionHandle> {
        // TODO: Seal and boot microVM
        Ok(PySessionHandle {})
    }
}

#[pymodule]
fn _kelir(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKelirBuilder>()?;
    m.add_class::<PySessionHandle>()?;
    Ok(())
}
