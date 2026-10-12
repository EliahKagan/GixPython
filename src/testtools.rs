use std::path::{Component, Path, PathBuf};

use pyo3::{
    exceptions::{PyFileNotFoundError, PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyBytes,
};

use crate::error::to_py;

#[pyclass(frozen, module = "gix.testtools")]
pub struct FixtureState {
    path: PathBuf,
    uninitialized: bool,
}

#[pymethods]
impl FixtureState {
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    fn is_uninitialized(&self) -> bool {
        self.uninitialized
    }
}

#[pyclass(eq, eq_int, module = "gix.testtools", from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Creation {
    CopyFromReadOnly,
    Execute,
}

impl From<Creation> for gix_testtools::Creation {
    fn from(value: Creation) -> Self {
        match value {
            Creation::CopyFromReadOnly => Self::CopyFromReadOnly,
            Creation::Execute => Self::Execute,
        }
    }
}

#[pyclass(frozen, module = "gix.testtools")]
pub struct TempDir {
    inner: std::sync::Mutex<Option<gix_testtools::tempfile::TempDir>>,
}

impl From<gix_testtools::tempfile::TempDir> for TempDir {
    fn from(inner: gix_testtools::tempfile::TempDir) -> Self {
        Self {
            inner: std::sync::Mutex::new(Some(inner)),
        }
    }
}

#[pymethods]
impl TempDir {
    fn path(&self) -> PyResult<PathBuf> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|dir| dir.path().to_owned())
            .ok_or_else(|| PyRuntimeError::new_err("temporary fixture is closed"))
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let dir = self.inner.lock().unwrap_or_else(|e| e.into_inner()).take();
        py.detach(|| dir.map(|dir| dir.close()).transpose())
            .map(|_| ())
            .map_err(to_py)
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        slf.path()?;
        Ok(slf)
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: &Bound<'_, PyAny>,
        _exc: &Bound<'_, PyAny>,
        _tb: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.close(py)
    }
}

fn fixture_error(error: Box<dyn std::error::Error + Send + Sync>) -> PyErr {
    match error.downcast::<PyErr>() {
        Ok(error) => *error,
        Err(error) => to_py(error.as_ref()),
    }
}

fn callback(callback: &Py<PyAny>, state: gix_testtools::FixtureState<'_>) -> gix_testtools::PostResult<Py<PyAny>> {
    Python::attach(|py| {
        let state = FixtureState {
            path: state.path().to_owned(),
            uninitialized: state.is_uninitialized(),
        };
        callback.call1(py, (state,))
    })
    .map_err(|error| Box::new(error) as _)
}

fn check_callback(py: Python<'_>, callback: &Py<PyAny>) -> PyResult<()> {
    if !callback.bind(py).is_callable() {
        return Err(PyTypeError::new_err("fixture post-processing must be callable"));
    }
    Ok(())
}

fn check_script(script_name: &Path) -> PyResult<()> {
    let path = gix_testtools::fixture_path(script_name);
    std::fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            PyFileNotFoundError::new_err(format!("{}: {error}", path.display()))
        } else {
            to_py(error)
        }
    })?;
    Ok(())
}

fn check_name(name: &str) -> PyResult<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(PyValueError::new_err("fixture name must be one normal path component"));
    }
    Ok(())
}

#[pyfunction]
fn fixture_path(path: PathBuf) -> PathBuf {
    gix_testtools::fixture_path(path)
}

#[pyfunction]
fn fixture_bytes(py: Python<'_>, path: PathBuf) -> PyResult<Bound<'_, PyBytes>> {
    let bytes = py
        .detach(|| std::fs::read(gix_testtools::fixture_path(path)))
        .map_err(to_py)?;
    Ok(PyBytes::new(py, &bytes))
}

#[pyfunction]
fn fixture_has_symlinks(py: Python<'_>, fixture_root: PathBuf) -> PyResult<bool> {
    py.detach(|| gix_testtools::fixture_has_symlinks(fixture_root))
        .map_err(to_py)
}

#[pyfunction]
fn scripted_fixture_read_only(py: Python<'_>, script_name: PathBuf) -> PyResult<PathBuf> {
    check_script(&script_name)?;
    py.detach(|| gix_testtools::scripted_fixture_read_only(script_name))
        .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_read_only_needs_archive(py: Python<'_>, script_name: PathBuf) -> PyResult<PathBuf> {
    check_script(&script_name)?;
    py.detach(|| gix_testtools::scripted_fixture_read_only_needs_archive(script_name))
        .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_read_only_with_args(py: Python<'_>, script_name: PathBuf, args: Vec<String>) -> PyResult<PathBuf> {
    check_script(&script_name)?;
    py.detach(|| gix_testtools::scripted_fixture_read_only_with_args(script_name, args))
        .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_read_only_with_post(
    py: Python<'_>,
    script_name: PathBuf,
    version: u32,
    post_process: Py<PyAny>,
) -> PyResult<(PathBuf, Py<PyAny>)> {
    check_script(&script_name)?;
    check_callback(py, &post_process)?;
    py.detach(|| {
        gix_testtools::scripted_fixture_read_only_with_post(script_name, version, |state| {
            callback(&post_process, state)
        })
    })
    .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_read_only_with_args_with_post(
    py: Python<'_>,
    script_name: PathBuf,
    args: Vec<String>,
    version: u32,
    post_process: Py<PyAny>,
) -> PyResult<(PathBuf, Py<PyAny>)> {
    check_script(&script_name)?;
    check_callback(py, &post_process)?;
    py.detach(|| {
        gix_testtools::scripted_fixture_read_only_with_args_with_post(script_name, args, version, |state| {
            callback(&post_process, state)
        })
    })
    .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_writable(py: Python<'_>, script_name: PathBuf) -> PyResult<TempDir> {
    check_script(&script_name)?;
    py.detach(|| gix_testtools::scripted_fixture_writable(script_name))
        .map(Into::into)
        .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_writable_with_args(
    py: Python<'_>,
    script_name: PathBuf,
    args: Vec<String>,
    mode: Creation,
) -> PyResult<TempDir> {
    check_script(&script_name)?;
    py.detach(|| gix_testtools::scripted_fixture_writable_with_args(script_name, args, mode.into()))
        .map(Into::into)
        .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_writable_with_post(
    py: Python<'_>,
    script_name: PathBuf,
    version: u32,
    post_process: Py<PyAny>,
) -> PyResult<(TempDir, Py<PyAny>)> {
    check_script(&script_name)?;
    check_callback(py, &post_process)?;
    py.detach(|| {
        gix_testtools::scripted_fixture_writable_with_post(script_name, version, |state| callback(&post_process, state))
    })
    .map(|(dir, value)| (dir.into(), value))
    .map_err(fixture_error)
}

#[pyfunction]
fn scripted_fixture_writable_with_args_with_post(
    py: Python<'_>,
    script_name: PathBuf,
    args: Vec<String>,
    mode: Creation,
    version: u32,
    post_process: Py<PyAny>,
) -> PyResult<(TempDir, Py<PyAny>)> {
    check_script(&script_name)?;
    check_callback(py, &post_process)?;
    py.detach(|| {
        gix_testtools::scripted_fixture_writable_with_args_with_post(script_name, args, mode.into(), version, |state| {
            callback(&post_process, state)
        })
    })
    .map(|(dir, value)| (dir.into(), value))
    .map_err(fixture_error)
}

#[pyfunction]
fn rust_fixture_read_only(
    py: Python<'_>,
    name: String,
    version: u32,
    make_fixture: Py<PyAny>,
) -> PyResult<(PathBuf, Py<PyAny>)> {
    check_name(&name)?;
    check_callback(py, &make_fixture)?;
    py.detach(|| gix_testtools::rust_fixture_read_only(&name, version, |state| callback(&make_fixture, state)))
        .map_err(fixture_error)
}

#[pyfunction]
fn rust_fixture_writable(
    py: Python<'_>,
    name: String,
    version: u32,
    mode: Creation,
    make_fixture: Py<PyAny>,
) -> PyResult<(TempDir, Py<PyAny>)> {
    check_name(&name)?;
    check_callback(py, &make_fixture)?;
    py.detach(|| {
        gix_testtools::rust_fixture_writable(&name, version, mode.into(), |state| callback(&make_fixture, state))
    })
    .map(|(dir, value)| (dir.into(), value))
    .map_err(fixture_error)
}

#[pyfunction]
fn git(py: Python<'_>, current_dir: PathBuf, arguments: String) -> PyResult<String> {
    py.detach(|| gix_testtools::git(current_dir, &arguments))
        .map_err(fixture_error)
}

pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    // Freeze the native tempfile registry without handlers before fixture helpers request signal setup.
    gix::tempfile::signal::setup(gix::tempfile::signal::handler::Mode::None);
    let m = PyModule::new(parent.py(), "gix._testtools")?;
    m.add_class::<FixtureState>()?;
    m.add_class::<Creation>()?;
    m.add_class::<TempDir>()?;
    macro_rules! functions {
        ($($name:ident),* $(,)?) => { $(m.add_function(wrap_pyfunction!($name, &m)?)?;)* };
    }
    functions!(
        fixture_path,
        fixture_bytes,
        fixture_has_symlinks,
        scripted_fixture_read_only,
        scripted_fixture_read_only_needs_archive,
        scripted_fixture_read_only_with_args,
        scripted_fixture_read_only_with_post,
        scripted_fixture_read_only_with_args_with_post,
        scripted_fixture_writable,
        scripted_fixture_writable_with_args,
        scripted_fixture_writable_with_post,
        scripted_fixture_writable_with_args_with_post,
        rust_fixture_read_only,
        rust_fixture_writable,
        git
    );
    parent
        .py()
        .import("sys")?
        .getattr("modules")?
        .set_item("gix._testtools", &m)?;
    Ok(())
}
