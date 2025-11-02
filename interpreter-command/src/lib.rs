use pyo3::prelude::*;
use pyo3::exceptions::{PyException, PyNotImplementedError};
use pyo3::{create_exception, intern};
use pyo3::types::{PyList, PyNone};

use ailang::{
    Value,
    AiCompiler,
    AiInterpreter,
    // Program,
    InterpreterState as AiInterpreterState,
    Op,
    Prop,
    Callable,
    CallableGenerator,
    Error,
    Arg,
};

create_exception!(ailangpy, AiError, PyException);

// REVISIT This kind of sucks as a means of error handling (deeply non-specific), but it works for
// now.
macro_rules! map_pyerr {
    ($expr:expr) => {
        $expr.map_err(|e| AiError::new_err(e.to_string()))
    }
}

macro_rules! map_foreign {
    ($py:expr, $expr:expr) => {
        $expr.map_err(|e: PyErr| Error::Foreign(e.value($py).repr().unwrap().extract().unwrap()))
    }
}

struct AiValue(Value);

impl std::ops::Deref for AiValue {
    type Target = Value;
    fn deref(&self) -> &Value {
        &self.0
    }
}

impl std::ops::DerefMut for AiValue {
    fn deref_mut(&mut self) -> &mut Value {
        &mut self.0
    }
}

impl<'a, 'py> pyo3::conversion::FromPyObject<'a, 'py> for AiValue {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(b) = obj.extract::<bool>() {
            Ok(AiValue(Value::Bool(b)))
        } else if let Ok(num) = obj.extract::<f64>() {
            Ok(AiValue(Value::Number(num)))
        } else if let Ok(_) = obj.cast::<pyo3::types::PyNone>() {
            Ok(AiValue(Value::Nil))
        } else if let Ok(s) = obj.extract::<String>() {
            Ok(AiValue(Value::String(s)))
        }else {
            Err(pyo3::exceptions::PyTypeError::new_err("Only primitive types allowed"))
        }
    }
}

impl<'py> pyo3::conversion::IntoPyObject<'py>  for AiValue {
    type Target = PyAny;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self.0 {
            Value::Bool(v) => v.into_pyobject(py).map(|b| b.to_owned().into_any()),
            Value::Number(v) => v.into_pyobject(py).map(|b| b.to_owned().into_any()),
            Value::String(v) => v.into_pyobject(py).map(|b| b.to_owned().into_any()),
            Value::Nil => Ok(PyNone::get(py).to_owned().into_any()),
        }
    }
}

macro_rules! not_impl {
    ($name:expr) => {
        Err(PyNotImplementedError::new_err(format!("'{}' is not implemnted", $name)))
    }
}

#[pyclass(name = "Callable", subclass)]
struct AiCallable;

#[pymethods]
impl AiCallable {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    #[allow(unused_variables)]
    fn new(args: &Bound<'_, PyAny>, kwargs: Option<&Bound<'_, PyAny>>) -> AiCallable {
        AiCallable
    }

    fn call(&mut self) -> PyResult<bool> {
        not_impl!("call")
    }
    fn terminate(&mut self) -> PyResult<()> {
        // not_impl!("terminate")
        // No-op by default should be a sensible default. I'll change that if it becomes an issue.
        Ok(())
    }
}

#[pyclass(name = "CallableGenerator", subclass)]
struct AiCallableGenerator;

#[pymethods]
impl AiCallableGenerator {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    #[allow(unused_variables)]
    fn new(args: &Bound<'_, PyAny>, kwargs: Option<&Bound<'_, PyAny>>) -> AiCallableGenerator {
        AiCallableGenerator
    }

    fn generate(&mut self) -> PyResult<AiCallable> {
        // Should this just call the constructor by default?
        // That's likely what it will be for a majority of classes.
        not_impl!("generate")
    }
    #[allow(unused_variables)]
    fn check_syntax(&self, args: Bound<'_, PyList>) -> PyResult<()> {
        not_impl!("check_syntax")
    }
}

#[pyclass(name = "Arg")]
struct AiArg(Arg);

impl From<Arg> for AiArg {
    fn from(value: Arg) -> AiArg {
        AiArg(value)
    }
}

#[pymethods]
impl AiArg {
    fn is_value(&self) -> bool {
        if let Arg::Value = self.0 {true} else {false}
    }
    
    fn is_word(&self) -> bool {
        if let Arg::Word(_) = self.0 {true} else {false}
    }

    fn matches_word(&self, s: &str) -> bool {
        if let Arg::Word(w) = &self.0 {w == s} else {false}
    }
}

struct PyCallable(Py<PyAny>);
struct PyCallableGenerator(Py<PyAny>);

impl CallableGenerator for PyCallableGenerator {
    fn generate(&mut self, args: Vec<Value>) -> Result<Box<dyn Callable>, Error> {
        Python::attach(|py| {
            let py_args = PyList::empty(py);
            for arg in args {
                map_foreign!(py, py_args.append(AiValue(arg)))?;
            }
            let res = map_foreign!(py, self.0.call_method1(py, intern!(py, "generate"), (py_args,)))?;
            let py_res = res.bind(py);
            if !py_res.is_instance_of::<AiCallable>() {
                let class = map_foreign!(py, py_res.get_type().name())?;
                let class_name = map_foreign!(py, class.extract())?;
                return Err(Error::InvalidCallable(class_name));
            }
            let res: Result<Box<dyn Callable>, Error> = Ok(Box::new(PyCallable(res)));
            res
        })
    }

    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error> {
        Python::attach(|py| {
            let py_params = PyList::empty(py);
            for arg in args {
                let _ = py_params.append(AiArg(arg));
            }
            map_foreign!(py, self.0.call_method1(py, intern!(py, "check_syntax"), (py_params,)))?;
            Ok(())
        })
    }
}

impl Callable for PyCallable {
    fn call(&mut self) -> Result<bool, Error> {
        Python::attach(|py| {
            let res = map_foreign!(py, self.0.call_method0(py, intern!(py, "call")))?;
            if res.bind(py).is_none() {
                return Ok(true);
            }
            map_foreign!(py, res.extract(py))
        })
    }

    fn terminate(&mut self) -> Result<(), Error> {
        Python::attach(|py| {
            map_foreign!(py, self.0.call_method0(py, intern!(py, "terminate")))?;
            Ok(())
        })
    }
}

#[pyclass(name = "Prop", subclass)]
struct AiProp;

#[pymethods]
impl AiProp {
    #[new]
    fn new() -> AiProp {
        AiProp
    }

    fn get(&self) -> PyResult<AiValue> {
        not_impl!("get")
    }

    fn set(&self) -> PyResult<AiValue> {
        not_impl!("set")
    }

    fn settable(&self) -> PyResult<bool> {
        // Unsettable is intended to be the default. Most props are going to be read-only
        Ok(false)
    }
}

struct PyProp(Py<PyAny>);

impl Prop for PyProp {
    fn get(&self) -> Result<Value, Error> {
        Python::attach(|py| {
            let py_res = map_foreign!(py, self.0.call_method0(py, intern!(py, "get")))?;
            let res: AiValue = map_foreign!(py, py_res.extract(py))?;
            Ok(res.0)
        })
    }

    fn set(&mut self, value: Value) -> Result<(), Error> {
        Python::attach(|py| {
            map_foreign!(py, self.0.call_method1(py, intern!(py, "set"), (AiValue(value),)))?;
            Ok(())
        })
    }

    fn settable(&self) -> Result<bool, Error> {
        Python::attach(|py| {
            let py_res = map_foreign!(py, self.0.call_method0(py, intern!(py, "is_settable")))?;
            map_foreign!(py, py_res.extract(py))
        })
    }
}



#[pyclass]
struct Compiler {
    compiler: AiCompiler,
    program: Option<Vec<Op>>,
    errors: Vec<Error>,
}

#[pymethods]
impl Compiler {
    #[new]
    fn new() -> Compiler {
        Compiler {
            compiler: AiCompiler::new(),
            program: None,
            errors: Vec::new(),
        }
    }

    #[staticmethod]
    fn check_if_callable(value: &Bound<'_, PyAny>) -> bool {
        value.is_instance_of::<AiCallableGenerator>()
    }

    fn register_callable(&mut self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !Self::check_if_callable(&value) {
            return Err(AiError::new_err("Not an instance of ailangpy.CallableGenerator"));
        }

        map_pyerr!(self.compiler.register_callable(name, PyCallableGenerator(value.unbind())))
    }

    #[staticmethod]
    fn check_if_prop(value: &Bound<'_, PyAny>) -> bool {
        value.is_instance_of::<AiProp>()
    }

    fn register_property(&mut self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !Self::check_if_prop(&value) {
            return Err(AiError::new_err("Not an instance of ailangpy.Prop"));
        }

        map_pyerr!(self.compiler.register_property(name, PyProp(value.unbind())))
    }

    fn compile(&mut self, py: Python<'_>, source: &str) -> bool {
        py.detach(|| {
            self.errors.clear();
            self.program = None;
            match self.compiler.compile_nonconsuming(source) {
                Ok(program) => {
                    self.program = Some(program);
                    true
                }
                Err(es) => {
                    self.errors = es;
                    false
                }
            }
        })
    }

    fn print_errors(&self) {
        for err in self.errors.iter() {
            println!("{}", err);
        }
    }

    fn into_interpreter(&mut self) -> PyResult<Interpreter> {
        if self.program.is_none() {
            return Err(AiError::new_err("No program to convert"));
        }
        let program = self.compiler.package_program(std::mem::take(self.program.as_mut().unwrap()));

        Ok(Interpreter(AiInterpreter::from_program(program)))
    }
}

#[pyclass]
enum InterpreterState {
    Yield,
    Stop,
}

impl From<AiInterpreterState> for InterpreterState {
    fn from(state: AiInterpreterState) -> Self {
        match state {
            AiInterpreterState::Yield => Self::Yield,
            AiInterpreterState::Stop => Self::Stop,
        }
    }
}

#[pyclass]
struct Interpreter(AiInterpreter);

#[pymethods]
impl Interpreter {
    #[new]
    #[pyo3(signature = (ir = None))]
    fn new(ir: Option<&str>) -> PyResult<Interpreter> {
        let terp = if let Some(ir) = ir {
            map_pyerr!(AiInterpreter::from_ir(ir))?
        } else {
            AiInterpreter::new(vec![])
        };
        Ok(Interpreter(terp))
    }

    fn reset(&mut self) -> PyResult<()> {
        map_pyerr!(self.0.reset())
    }

    fn stop(&mut self) -> PyResult<()> {
        map_pyerr!(self.0.end())
    }

    fn run(&mut self) -> PyResult<bool> {
        map_pyerr!(self.0.step().map(|s| s == AiInterpreterState::Yield))
    }
}


// 
// /// Formats the sum of two numbers as string.
// #[pyfunction]
// fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
//     Ok((a + b).to_string())
// }
// 
// #[pyfunction]
// fn print_value(value: AiValue) {
//     println!("{}", *value);
// }

/// A Python module implemented in Rust.
#[pymodule(name = "ailangpy")]
fn load_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    // m.add_function(wrap_pyfunction!(print_value, m)?)?;

    m.add_class::<Interpreter>()?;
    m.add_class::<Compiler>()?;
    m.add_class::<InterpreterState>()?;
    m.add_class::<AiArg>()?;
    m.add_class::<AiCallable>()?;
    m.add_class::<AiCallableGenerator>()?;
    m.add_class::<AiProp>()?;
    Ok(())
}
