use pyo3::exceptions::PyValueError;
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyModule, PyModuleMethods,
};
use pyo3::{
    pyfunction, pymodule, wrap_pyfunction, Bound, Py, PyAny, PyResult, Python,
};

/// Python module: _rust_parse
#[pymodule]
fn _rust_parse(_: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_repair_json, m)?)?;
    m.add_function(wrap_pyfunction!(py_format_json, m)?)?;
    m.add_function(wrap_pyfunction!(py_repair_json_with_trace, m)?)?;
    Ok(())
}

/// Repair malformed JSON text and return a valid JSON string.
///
/// This is the low-level PyO3 binding.  Python users should call
/// ``json_repair.repair_json`` instead, which wraps this function and adds
/// empty-input handling, surrogate stripping, and optional object return.
///
/// ``max_depth`` optionally overrides the default nesting limit.  When
/// ``None`` the Rust default (`512`) is used.
///
/// Releases the Python GIL during the Rust computation so other Python
/// threads can run concurrently.
#[pyfunction]
#[pyo3(signature = (text, max_depth=None))]
fn py_repair_json(py: Python<'_>, text: &str, max_depth: Option<usize>) -> PyResult<String> {
    py.detach(move || {
        let config = match max_depth {
            Some(d) => json_repair_core::RepairConfig::default().with_max_depth(d),
            None => json_repair_core::RepairConfig::default(),
        };
        json_repair_core::repair_json_with(text, &config)
    })
    .map_err(|e| PyValueError::new_err(format!("{e}")))
}

/// Pretty-print a valid JSON string with configurable indentation.
///
/// ``indent`` is the number of spaces per nesting level. This is the
/// low-level PyO3 binding; Python users should call
/// ``json_repair.repair_json(..., indent=N)`` instead.
///
/// Releases the Python GIL during the Rust computation.
#[pyfunction]
#[pyo3(signature = (text, indent))]
fn py_format_json(py: Python<'_>, text: &str, indent: usize) -> PyResult<String> {
    py.detach(move || json_repair_core::format_json(text, indent))
        .map_err(|e| PyValueError::new_err(format!("{e}")))
}

/// Repair malformed JSON text and return the repaired string plus a trace.
///
/// This is the low-level PyO3 binding for the ``tracing`` feature.  Python
/// users should call ``json_repair.repair_json(..., traced=True)`` instead.
///
/// The returned trace is a dict with an ``events`` key containing a list of
/// dicts, one per recorded repair event.
///
/// ``max_depth`` optionally overrides the default nesting limit.  When
/// ``None`` the Rust default (``512``) is used.
///
/// Releases the Python GIL during the Rust computation.
#[pyfunction]
#[pyo3(signature = (text, max_depth=None))]
fn py_repair_json_with_trace(
    py: Python<'_>,
    text: &str,
    max_depth: Option<usize>,
) -> PyResult<(String, Py<PyAny>)> {
    let config = json_repair_core::RepairConfig::default()
        .with_tracing(true)
        .with_max_depth(max_depth.unwrap_or(json_repair_core::DEFAULT_MAX_PARSE_DEPTH));
    let (json, trace) = py
        .detach(move || json_repair_core::repair_json_with_trace(text, &config))
        .map_err(|e| PyValueError::new_err(format!("{e}")))?;
    Ok((json, trace_to_py(py, &trace)?))
}

/// Convert a Rust [`json_repair_core::RepairTrace`] into a Python dict.
fn trace_to_py(py: Python<'_>, trace: &json_repair_core::RepairTrace) -> PyResult<Py<PyAny>> {
    use json_repair_core::{CommentStyle, TraceEvent};

    let events = PyList::empty(py);
    for event in trace.events() {
        let dict = PyDict::new(py);
        match event {
            TraceEvent::CommentSkipped { style, start } => {
                dict.set_item("type", "CommentSkipped")?;
                dict.set_item(
                    "style",
                    match style {
                        CommentStyle::Line => "Line",
                        CommentStyle::Block => "Block",
                        CommentStyle::Hash => "Hash",
                        CommentStyle::DashDash => "DashDash",
                    },
                )?;
                dict.set_item("start", *start)?;
            }
            TraceEvent::StringSplit { position, reason } => {
                dict.set_item("type", "StringSplit")?;
                dict.set_item("position", *position)?;
                dict.set_item("reason", *reason)?;
            }
            TraceEvent::ContainerClosed {
                bracket,
                forced_at_eof,
            } => {
                dict.set_item("type", "ContainerClosed")?;
                dict.set_item("bracket", bracket.to_string())?;
                dict.set_item("forced_at_eof", *forced_at_eof)?;
            }
            TraceEvent::ImplicitNull { key_position } => {
                dict.set_item("type", "ImplicitNull")?;
                dict.set_item("key_position", *key_position)?;
            }
            TraceEvent::ImplicitArrayDetected { reason } => {
                dict.set_item("type", "ImplicitArrayDetected")?;
                dict.set_item("reason", *reason)?;
            }
            TraceEvent::ValueNormalized { kind } => {
                dict.set_item("type", "ValueNormalized")?;
                dict.set_item("kind", *kind)?;
            }
            TraceEvent::MismatchedBracket { expected, found } => {
                dict.set_item("type", "MismatchedBracket")?;
                dict.set_item("expected", expected.map(|c| c.to_string()))?;
                dict.set_item("found", found.to_string())?;
            }
        }
        events.append(dict)?;
    }

    let result = PyDict::new(py);
    result.set_item("events", events)?;
    Ok(result.into())
}
