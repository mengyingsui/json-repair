use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Python module: _rust_parse
#[pymodule]
fn _rust_parse(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_repair_json, m)?)?;
    Ok(())
}

/// Repair malformed JSON text and return a valid JSON string.
#[pyfunction]
fn py_repair_json(text: &str) -> PyResult<String> {
    match json_repair_core::repair_json(text) {
        Ok(s) => Ok(s),
        Err(e) => Err(PyValueError::new_err(format!("{e}"))),
    }
}
