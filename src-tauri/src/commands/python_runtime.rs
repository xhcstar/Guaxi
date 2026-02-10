use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyModule};
use pyo3::IntoPyObject;
use std::ffi::CString;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::command;
use tracing::{error, info, warn};

static PY_READY: OnceLock<()> = OnceLock::new();
static PY_REPL_GLOBALS: OnceLock<Mutex<Option<Py<PyDict>>>> = OnceLock::new();
static PY_SESSION_GLOBALS: OnceLock<Mutex<HashMap<String, Py<PyDict>>>> = OnceLock::new();

fn ensure_python_ready() {
    PY_READY.get_or_init(|| {
        pyo3::prepare_freethreaded_python();
        info!("[PythonRuntime] prepare_freethreaded_python done");
    });
}

#[derive(Debug, Deserialize)]
pub struct PyExecRequest {
    pub code: String,
    pub persist: bool,
    pub expect_json: bool,
    pub vars: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize)]
pub struct PyExecResponse {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub result_repr: Option<String>,
    pub result_json: Option<Value>,
}

fn build_fresh_globals(py: Python<'_>) -> PyResult<Py<PyDict>> {
    let g = PyDict::new(py);
    let builtins = PyModule::import(py, "builtins")?;
    g.set_item("__builtins__", builtins)?;
    Ok(g.unbind())
}

fn get_or_create_repl_globals(py: Python<'_>) -> PyResult<Py<PyDict>> {
    let cell = PY_REPL_GLOBALS.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to lock repl globals")
    })?;

    if let Some(existing) = guard.as_ref() {
        return Ok(existing.clone_ref(py));
    }

    let created = build_fresh_globals(py)?;
    *guard = Some(created.clone_ref(py));
    Ok(created)
}

fn get_or_create_session_globals(py: Python<'_>, session_id: &str) -> PyResult<Py<PyDict>> {
    let sid = session_id.trim();
    if sid.is_empty() {
        return get_or_create_repl_globals(py);
    }

    let cell = PY_SESSION_GLOBALS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cell.lock().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Failed to lock session globals")
    })?;

    if let Some(existing) = guard.get(sid) {
        return Ok(existing.clone_ref(py));
    }

    let created = build_fresh_globals(py)?;
    guard.insert(sid.to_string(), created.clone_ref(py));
    Ok(created)
}

fn format_pyerr_traceback(py: Python<'_>, err: &PyErr) -> String {
    let tb_mod = PyModule::import(py, "traceback");
    let tb_mod = match tb_mod {
        Ok(v) => v,
        Err(_) => return format!("{err}"),
    };

    let f = tb_mod.getattr("format_exception");
    let f = match f {
        Ok(v) => v,
        Err(_) => return format!("{err}"),
    };

    // Python 3.11+: traceback.format_exception(exc) 允许只传入异常实例
    if let Ok(lines) = f
        .call1((err.value(py),))
        .and_then(|o| o.extract::<Vec<String>>())
    {
        let joined = lines.join("");
        if !joined.trim().is_empty() {
            return joined;
        }
    }

    // 兼容旧签名：traceback.format_exception(type, value, tb)
    let typ = err.get_type(py);
    let val = err.value(py);
    let tb: PyObject = err
        .traceback(py)
        .map(|t| t.into_any().unbind())
        .unwrap_or_else(|| py.None());

    if let Ok(lines) = f
        .call1((typ, val, tb))
        .and_then(|o| o.extract::<Vec<String>>())
    {
        let joined = lines.join("");
        if !joined.trim().is_empty() {
            return joined;
        }
    }

    format!("{err}")
}

fn json_to_py(py: Python<'_>, v: &Value) -> PyObject {
    match v {
        Value::Null => py.None(),
        Value::Bool(b) => PyBool::new(py, *b).to_owned().into_any().unbind(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                match i.into_pyobject(py) {
                    Ok(obj) => obj.into_any().unbind(),
                    Err(e) => {
                        warn!(err = %e, "[PythonRuntime] convert i64 to PyObject failed");
                        py.None()
                    }
                }
            } else if let Some(u) = n.as_u64() {
                match u.into_pyobject(py) {
                    Ok(obj) => obj.into_any().unbind(),
                    Err(e) => {
                        warn!(err = %e, "[PythonRuntime] convert u64 to PyObject failed");
                        py.None()
                    }
                }
            } else if let Some(f) = n.as_f64() {
                match f.into_pyobject(py) {
                    Ok(obj) => obj.into_any().unbind(),
                    Err(e) => {
                        warn!(err = %e, "[PythonRuntime] convert f64 to PyObject failed");
                        py.None()
                    }
                }
            } else {
                py.None()
            }
        }
        Value::String(s) => match s.as_str().into_pyobject(py) {
            Ok(obj) => obj.into_any().unbind(),
            Err(e) => {
                warn!(err = %e, "[PythonRuntime] convert string to PyObject failed");
                py.None()
            }
        },
        Value::Array(arr) => {
            let items: Vec<PyObject> = arr.iter().map(|x| json_to_py(py, x)).collect();
            match PyList::new(py, items) {
                Ok(list) => list.unbind().into_any(),
                Err(e) => {
                    warn!(err = %e, "[PythonRuntime] build PyList failed");
                    py.None()
                }
            }
        }
        Value::Object(map) => {
            let d = PyDict::new(py);
            for (k, vv) in map {
                let _ = d.set_item(k, json_to_py(py, vv));
            }
            d.unbind().into_any()
        }
    }
}

fn py_to_json_basic(py: Python<'_>, obj: &Bound<'_, pyo3::types::PyAny>) -> Option<Value> {
    if obj.is_none() {
        return Some(Value::Null);
    }
    if let Ok(v) = obj.extract::<bool>() {
        return Some(Value::Bool(v));
    }
    if let Ok(v) = obj.extract::<i64>() {
        return Some(Value::Number(v.into()));
    }
    if let Ok(v) = obj.extract::<u64>() {
        return Some(Value::Number(v.into()));
    }
    if let Ok(v) = obj.extract::<f64>() {
        return serde_json::Number::from_f64(v).map(Value::Number);
    }
    if let Ok(v) = obj.extract::<String>() {
        return Some(Value::String(v));
    }

    if let Ok(list) = obj.downcast::<PyList>() {
        let mut out = Vec::with_capacity(list.len());
        for it in list.iter() {
            out.push(py_to_json_basic(py, &it).unwrap_or(Value::Null));
        }
        return Some(Value::Array(out));
    }

    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut out = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let ks = k.extract::<String>().ok()?;
            out.insert(ks, py_to_json_basic(py, &v).unwrap_or(Value::Null));
        }
        return Some(Value::Object(out));
    }

    None
}

fn capture_stdio(py: Python<'_>) -> PyResult<(PyObject, PyObject, PyObject, PyObject)> {
    let sys = PyModule::import(py, "sys")?;
    let io = PyModule::import(py, "io")?;
    let string_io = io.getattr("StringIO")?;

    let old_stdout = sys.getattr("stdout")?.unbind();
    let old_stderr = sys.getattr("stderr")?.unbind();

    let new_stdout = string_io.call0()?.unbind();
    let new_stderr = string_io.call0()?.unbind();

    sys.setattr("stdout", new_stdout.bind(py))?;
    sys.setattr("stderr", new_stderr.bind(py))?;

    Ok((old_stdout, old_stderr, new_stdout, new_stderr))
}

fn restore_stdio(py: Python<'_>, old_stdout: PyObject, old_stderr: PyObject) -> PyResult<()> {
    let sys = PyModule::import(py, "sys")?;
    sys.setattr("stdout", old_stdout.bind(py))?;
    sys.setattr("stderr", old_stderr.bind(py))?;
    Ok(())
}

fn read_stringio(py: Python<'_>, sio: &PyObject) -> PyResult<String> {
    let v: String = sio
        .bind(py)
        .getattr("getvalue")?
        .call0()?
        .extract()?;
    Ok(v)
}

fn should_inline_matplotlib(vars: Option<&HashMap<String, Value>>) -> bool {
    if let Some(v) = vars.and_then(|m| m.get("_guaxi_inline_matplotlib")) {
        if let Some(b) = v.as_bool() {
            return b;
        }
    }

    if let Some(m) = vars {
        return m.contains_key("_py_cell_id") || m.contains_key("_guaxi_session");
    }

    false
}

fn try_prepare_inline_matplotlib(py: Python<'_>, globals: &Bound<'_, PyDict>) {
    let pre = r#"
try:
    import sys
    if 'matplotlib' not in sys.modules:
        import matplotlib
        matplotlib.use('Agg')
except Exception:
    pass
"#;

    let Ok(c) = CString::new(pre) else {
        return;
    };

    if let Err(e) = py.run(c.as_c_str(), Some(globals), Some(globals)) {
        warn!(err = %e, "[PythonRuntime] prepare inline matplotlib failed");
    }
}

fn try_capture_matplotlib_png_base64(py: Python<'_>, globals: &Bound<'_, PyDict>) -> Option<String> {
    let cap = r#"
__guaxi_mpl_png_base64 = None
try:
    import sys
    if 'matplotlib' in sys.modules:
        import base64
        from io import BytesIO
        from matplotlib._pylab_helpers import Gcf

        mgrs = Gcf.get_all_fig_managers()
        if mgrs:
            fig = mgrs[-1].canvas.figure
            buf = BytesIO()
            fig.savefig(buf, format='png', bbox_inches='tight')
            __guaxi_mpl_png_base64 = base64.b64encode(buf.getvalue()).decode('ascii')

            try:
                import matplotlib.pyplot as plt
                for m in mgrs:
                    try:
                        plt.close(m.canvas.figure)
                    except Exception:
                        pass
            except Exception:
                pass
except Exception:
    __guaxi_mpl_png_base64 = None
"#;

    let c = CString::new(cap).ok()?;
    if let Err(e) = py.run(c.as_c_str(), Some(globals), Some(globals)) {
        warn!(err = %e, "[PythonRuntime] capture matplotlib figure failed");
        let _ = globals.del_item("__guaxi_mpl_png_base64");
        return None;
    }

    let out = globals
        .get_item("__guaxi_mpl_png_base64")
        .ok()
        .flatten()
        .and_then(|v| v.extract::<Option<String>>().ok())
        .flatten();

    let _ = globals.del_item("__guaxi_mpl_png_base64");
    out
}

#[command]
pub async fn python_reset_repl() -> Result<(), String> {
    ensure_python_ready();
    let cell = PY_REPL_GLOBALS.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().map_err(|_| "Failed to lock repl globals".to_string())?;
    *guard = None;
    info!("[PythonRuntime] repl globals reset");
    Ok(())
}

#[command]
pub async fn python_exec(request: PyExecRequest) -> Result<PyExecResponse, String> {
    ensure_python_ready();

    let code_len = request.code.len();
    let persist = request.persist;
    let expect_json = request.expect_json;
    info!(
        persist = persist,
        expect_json = expect_json,
        code_len = code_len,
        "[PythonRuntime] python_exec called"
    );

    let task = tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| -> Result<PyExecResponse, String> {
            let (old_stdout, old_stderr, new_stdout, new_stderr) = capture_stdio(py)
                .map_err(|e| format!("capture stdio failed: {e}"))?;

            let session_id = request
                .vars
                .as_ref()
                .and_then(|m| m.get("_guaxi_session"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let globals_py = if request.persist {
                if let Some(sid) = session_id.as_deref() {
                    get_or_create_session_globals(py, sid)
                        .map_err(|e| format!("get session globals failed: {e}"))?
                } else {
                    get_or_create_repl_globals(py).map_err(|e| format!("get repl globals failed: {e}"))?
                }
            } else {
                build_fresh_globals(py).map_err(|e| format!("build globals failed: {e}"))?
            };

            let globals = globals_py.bind(py);

            let inline_matplotlib = should_inline_matplotlib(request.vars.as_ref());
            if inline_matplotlib {
                try_prepare_inline_matplotlib(py, globals);
            }

            if let Some(vars) = request.vars.as_ref() {
                for (k, v) in vars {
                    if let Err(e) = globals.set_item(k, json_to_py(py, v)) {
                        warn!(key = k.as_str(), err = %e, "[PythonRuntime] set var failed");
                    }
                }
            }

            let mut ok = true;
            let mut result_repr: Option<String> = None;
            let mut result_json: Option<Value> = None;

            if request.code.contains('\0') {
                return Err("Python code contains NUL byte (\\0), cannot execute".to_string());
            }

            if let Err(e) = globals.set_item("__guaxi_code", request.code.as_str()) {
                warn!(err = %e, "[PythonRuntime] set __guaxi_code failed");
            }

            let wrapper = r#"
import ast

__guaxi_pre_has_result = '_result' in globals()
__guaxi_code = globals().get('__guaxi_code', '')
__guaxi_m = ast.parse(__guaxi_code, mode='exec')
__guaxi_has_last_expr = bool(__guaxi_m.body) and isinstance(__guaxi_m.body[-1], ast.Expr)

if __guaxi_has_last_expr:
    __guaxi_last_expr = __guaxi_m.body.pop().value
    __guaxi_m.body.append(ast.Assign(
        targets=[ast.Name(id='__guaxi_last_value', ctx=ast.Store())],
        value=__guaxi_last_expr,
    ))
    ast.fix_missing_locations(__guaxi_m)

exec(compile(__guaxi_m, '<guaxi>', 'exec'), globals(), globals())

if __guaxi_has_last_expr and (not __guaxi_pre_has_result) and ('__guaxi_last_value' in globals()):
    globals()['_result'] = globals().get('__guaxi_last_value')

try:
    del globals()['__guaxi_last_value']
except Exception:
    pass
"#;

            let wrapper_c = CString::new(wrapper)
                .map_err(|_| "Internal error: wrapper contains NUL byte".to_string())?;

            let run_res = py.run(wrapper_c.as_c_str(), Some(globals), Some(globals));
            let mut err_trace = String::new();
            if let Err(e) = run_res {
                ok = false;
                err_trace = format_pyerr_traceback(py, &e);
                warn!("[PythonRuntime] python exec error: {:?}", e);
            }

            if ok {
                match globals.get_item("_result") {
                    Ok(Some(obj)) => {
                        if let Ok(r) = obj.repr() {
                            result_repr = Some(r.to_string());
                        }
                        if request.expect_json {
                            result_json = py_to_json_basic(py, &obj);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!("[PythonRuntime] read _result failed: {:?}", e);
                    }
                }

                if inline_matplotlib {
                    if let Some(png_b64) = try_capture_matplotlib_png_base64(py, globals) {
                        info!(len = png_b64.len(), "[PythonRuntime] captured matplotlib figure");
                        let display = json!({
                            "type": "image",
                            "mime": "image/png",
                            "data_base64": png_b64,
                        });

                        match result_json {
                            Some(Value::Object(ref mut map)) => {
                                if !map.contains_key("__guaxi_display__") {
                                    map.insert("__guaxi_display__".to_string(), display);
                                }
                            }
                            Some(other) => {
                                result_json = Some(json!({
                                    "__guaxi_display__": display,
                                    "__guaxi_result__": other,
                                }));
                            }
                            None => {
                                result_json = Some(json!({ "__guaxi_display__": display }));
                            }
                        }
                    }
                }
            }

            let stdout = read_stringio(py, &new_stdout).unwrap_or_default();
            let mut stderr = read_stringio(py, &new_stderr).unwrap_or_default();
            if !err_trace.trim().is_empty() {
                if !stderr.trim().is_empty() {
                    stderr.push_str("\n");
                }
                stderr.push_str(&err_trace);
            }

            if let Err(e) = restore_stdio(py, old_stdout, old_stderr) {
                error!("[PythonRuntime] restore stdio failed: {:?}", e);
            }

            Ok(PyExecResponse {
                ok,
                stdout,
                stderr,
                result_repr,
                result_json,
            })
        })
    });

    task.await
        .map_err(|e| format!("python_exec task join failed: {e}"))?
}