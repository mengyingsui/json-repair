use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Python module: _rust_parse
#[pymodule]
fn _rust_parse(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_repair_json, m)?)?;
    Ok(())
}

/// Repair malformed JSON text and return a valid JSON string.
///
/// Releases the Python GIL during the Rust computation so other Python
/// threads can run concurrently.
#[pyfunction]
fn py_repair_json(py: Python<'_>, text: &str) -> PyResult<String> {
    py.detach(move || json_repair_core::repair_json(text))
        .map_err(|e| PyValueError::new_err(format!("{e}")))
}
