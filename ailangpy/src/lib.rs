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

/// A superclass required for defining an Ai-compatible native callable. 
///
/// This is a virtual(-ish) class that does not have a valid implementation.
///
/// Subclasses of this type should never need to be instantiated directly. Instead, you 
/// should implement a `CallableGenerator` for this that creates instances of this class. 
/// For more information on behaviour, please refer to `CallableGenerator`.
///
/// Note that in some simple cases, it may be possible to make a class the subclass of both
/// `Callable` and `CallableGenerator`. This is perfectly valid.
#[pyclass(name = "Callable", subclass)]
struct AiCallable;

#[pymethods]
impl AiCallable {
    /// Creates a new instance of the `Callable`. The arguments to a native call in Ai are the
    /// values provided as arguments to the constructor (though a `CallableGenerator` is allowed to
    /// modify this behaviour in any way.)
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    #[allow(unused_variables)]
    fn new(args: &Bound<'_, PyAny>, kwargs: Option<&Bound<'_, PyAny>>) -> AiCallable {
        AiCallable
    }

    /// Called by the Ai runtime any time a native call associated with this type is active. A
    /// return value of True indicates the call is complete, while False causes the Ai interpreter to
    /// yield, calling this function again when execution resumes. A value of None is also valid
    /// and is treated the same as True (this simplifies writing simple `Callable`s).
    fn call(&mut self) -> PyResult<bool> {
        not_impl!("call")
    }
    
    /// Called by the Ai runtime whenever a native call associated with this type is to be ended
    /// early. This can happen if a race group containing native calls ends, or if the Ai interpreter
    /// itself is ended early.
    ///
    /// Note: This is **not** called on a runtime error in Ai. If a runtime error occurs, cleanup must be
    /// performed outside the context of the interpreter.
    fn terminate(&mut self) -> PyResult<()> {
        // not_impl!("terminate")
        // No-op by default should be a sensible default. I'll change that if it becomes an issue.
        Ok(())
    }
}

/// A superclass required for defining an Ai-compatible native callable.
///
/// This is a virtual(-ish) class that does not have a valid implementation.
///
/// The purpose of this class is to generate a new instance of each `Callable` whenever Ai
/// needs to execute a new native call. At this time, a new `Callable` will be generated each time
/// the native call is reached, (though not for each iteration of that native call). This is
/// because it is entirely possible with parallel groups to have multiple instances of a given 
/// callable running at once, even for the same instruction.
///
/// If this is undesirable, the generator can be designed to return a singleton-style `Callable`.
/// However, this is discouraged as it may cause issues with parallelism if the call is non-trivial.
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

    /// Called by the Ai runtime when it encounters a native call associated with this type.
    /// Generates a new `Callable` based on the arguments passed from Ai.
    ///
    /// Most generators will simply pass the arguments to this function straight to the constructor
    /// for the associated `Callable`, however this method can be used to insert arguments (as in
    /// some kind of dependency injection), or filter them out, or even dispatch to multiple different
    /// kinds of `Callable`s.
    ///
    /// Note: unlike `check_syntax` these arguments will be actual values, and only values. Any
    /// syntactical words are filtered out by the Ai compiler.
    #[allow(unused_variables)]
    fn generate(&mut self, args: Bound<'_, PyList>) -> PyResult<AiCallable> {
        // Should this just call the constructor by default?
        // That's likely what it will be for a majority of classes.
        not_impl!("generate")
    }

    /// Called by the Ai compiler to check if a call's syntax is considrered valid. It is
    /// considered valid if no exception was thrown. This allows for simple checking use Python's
    /// built-in `assert` to check type and count.
    ///
    /// For more information, check the documentation for the `Arg` type.
    ///
    /// Note: at this point, actual argument values are not known, just that there is a Value in
    /// that position. Words, however, only exist at compile-time, so their value is known and can
    /// be checked or even switched upon to provide a form of overloading (making this actually
    /// play nice at runtime is an exercise left to the user).
    #[allow(unused_variables)]
    fn check_syntax(&self, args: Bound<'_, PyList>) -> PyResult<()> {
        not_impl!("check_syntax")
    }
}

/// A class representing the compile-time concept of an argument. Used exclusively in
/// `CallableGenerator.check_syntax`.
///
/// An argument can be either a value or a word. A value represents an Ai runtime value, but it
/// can not currently hold that value, since Ai values don't exist at compile-time. A value can be
/// recognized using the `Arg.is_value` method. A word is anything that isn't a value, variable name, 
/// or reserved syntactical element. This *is* known at compile-time, and can be matched against 
/// using the `Arg.matches_word` method. If you only care about the presence of a word, you can use
/// the `Arg.is_word` method.
#[pyclass(name = "Arg")]
struct AiArg(Arg);

impl From<Arg> for AiArg {
    fn from(value: Arg) -> AiArg {
        AiArg(value)
    }
}

#[pymethods]
impl AiArg {
    /// Returns whether an Arg represents a value. The value itself is not known, just that it
    /// will eventually be *some* value.
    fn is_value(&self) -> bool {
        if let Arg::Value = self.0 {true} else {false}
    }
    
    /// Returns whether an Arg represents a word. Since words are known at compile-time,
    /// `Arg.matches_word` is more likely to be useful for checks.
    fn is_word(&self) -> bool {
        if let Arg::Word(_) = self.0 {true} else {false}
    }

    /// Returns whether both an Arg is a word, and that word matches the given string exactly. 
    /// 
    /// Checking is done using a simple byte-wise equality, so diacritics and other non-unique
    /// Unicode may complicate things.
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

/// A superclass required for defining an Ai property. This is a virtual(-ish) class that does 
/// not have a valid implementation.
///
/// A property is treated like a variable in the Ai runtime, but instead of getting or setting a
/// variable of the associated name, the `get` and `set` methods of this type are called,
/// respectively.
///
/// Having a Prop be settable is entirely optional, since assigning to a source (for example, a
/// sensor) doesn't always make sense. By default, a property is not settable, as determined by the
/// value returned by the `settable` method. This can be changed by overriding it.
///
/// If a property is not settable, the associated `Prop` does not need to define a `set` method, 
/// since the compiler checks for settability, and the interpreter validates IR before it's run.
///
/// Unlike `Callable`s, a property being accessed does not cause the Ai runtime to yield.
#[pyclass(name = "Prop", subclass)]
struct AiProp;

#[pymethods]
impl AiProp {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    #[allow(unused_variables)]
    fn new(args: &Bound<'_, PyAny>, kwargs: Option<&Bound<'_, PyAny>>) -> AiProp {
        AiProp
    }

    /// Retrieves a value from the property. The only allowed types are `int`, `float`, `str`,
    /// `bool` and `None`. Anything else will cause a runtime error.
    ///
    /// This method should never be called directly, as its return value is completely opaque and
    /// unusable in Python.
    fn get(&self) -> PyResult<AiValue> {
        not_impl!("get")
    }

    /// Sets the value of the property, for some definition of "set". Setting a property to a
    /// specific value does *not* imply that a subsequent call to `get` will return the same value.
    ///
    /// This method is only required if your program will set the value. There's no 
    /// syntactical way to prevent assignment, but an unassignable property will report errors on
    /// assignment at both compile-time and runtime.
    ///
    /// This method should never be called directly, as it is impossible to create the required
    /// value in Python.
    #[allow(unused_variables)]
    fn set(&self, value: Bound<'_, PyAny>) -> PyResult<()> {
        not_impl!("set")
    }

    /// Returns whether the property is settable. If a property is not settable, the `set` method
    /// does not need to be implemented, as it can never be called.
    fn is_settable(&self) -> PyResult<bool> {
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


/// A compiler for the Ai language. Performs syntax checking and name resolution, storing a
/// text-based intermediate representation (IR) that is executed by the Ai interpreter. A
/// `Compiler` can be converted directly to an `Interpreter` using the `into_interpreter` method, or it
/// can be converted into the raw IR using the `into_raw` method.
///
/// Note that in order to use custom `Callable`s and `Prop`s in a program, you must register them
/// with the compiler so that it can recognize them in a program. For more details , see the
/// respective `register_*` methods.
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

    /// Checks if a value is a subclass of `CallableGenerator`. 
    ///
    /// This exists primarily as a convenience function for the implementation, and should 
    /// be treated as deprecated, though it is unlikely to ever be removed. 
    #[staticmethod]
    fn check_if_callable(value: &Bound<'_, PyAny>) -> bool {
        value.is_instance_of::<AiCallableGenerator>()
    }

    /// Registers the given `CallableGenerator` under the provided name. If the value is not a
    /// subclass of `CallableGenerator`, an exception is thrown.
    ///
    /// In order for a name to be valid in a given program, it must be registered with an
    /// associated `CallableGenerator`. This generator is then invoked any time the name is
    /// referenced as a call. For more details on those mechanics, refer to `CallableGenerator`.
    fn register_callable(&mut self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !Self::check_if_callable(&value) {
            return Err(AiError::new_err("Not an instance of ailangpy.CallableGenerator"));
        }

        map_pyerr!(self.compiler.register_callable(name, PyCallableGenerator(value.unbind())))
    }

    /// Checks if a value is a subclass of `Prop`. 
    ///
    /// This exists primarily as a convenience function for the implementation, and should 
    /// be treated as deprecated, though it is unlikely to ever be removed. 
    #[staticmethod]
    fn check_if_prop(value: &Bound<'_, PyAny>) -> bool {
        value.is_instance_of::<AiProp>()
    }

    /// Registers the given `Prop` under the provided name. If the value is not a
    /// subclass of `Prop`, an exception is thrown. Note that the given name should not be preceded
    /// by a dollar sign ($ - U+0024), as that is handled internally by the compiler.
    ///
    /// In order for a name to be valid in a given program, it must be registered with an
    /// associated `Prop`. This instance's methods are then used any time that name 
    /// is referenced in the program. For more details on those mechanics, refer to `Prop`.
    fn register_property(&mut self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !Self::check_if_prop(&value) {
            return Err(AiError::new_err("Not an instance of ailangpy.Prop"));
        }

        map_pyerr!(self.compiler.register_property(name, PyProp(value.unbind())))
    }

    /// Compiles the provided program, storing the results internally, returning `True` if the
    /// program was compiled successfully, and False if there was some form of compilation error.
    ///
    /// If a `compile` is called more than once, previous results, both program and errors, are discarded.
    ///
    /// If any errors are raised by the compiler, they are also stored, and can be viewed using 
    /// `print_errors`. At present, there is no way to inspect the errors programatically.
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

    /// Displays any errors that occured during the most recent compilation. Prints nothing if
    /// there are no errors.
    fn print_errors(&self) {
        for err in self.errors.iter() {
            println!("{}", err);
        }
    }

    /// Checks if the compiler has been given a program and that it compiled successfully. Returns
    /// the same result as `compile`.
    fn has_program(&self) -> bool {
        self.program.is_some()
    }

    /// Effectively consumes the compiler, converting it into an `Interpreter`. Throws an exception
    /// if there is no valid program stored. 
    ///
    /// In principle, on success, this method completely consumes the compiler, moving all of its assets,
    /// including the registered callables and properties, over to the new interpreter, disallowing
    /// further access to the compiler. In practice, because of Python's memory model, a successful call 
    /// effectively just removes the contents of the compiler, functionally resetting it to its
    /// initial state. This means you will need to re-register all callables and properties to
    /// re-compile the same program.
    fn into_interpreter(&mut self) -> PyResult<Interpreter> {
        if self.program.is_none() {
            return Err(AiError::new_err("No program to convert"));
        }
        let program = self.compiler.package_program(std::mem::take(self.program.as_mut().unwrap()));

        Ok(Interpreter(AiInterpreter::from_program(program)))
    }

    /// Takes the most recent program and converts it to the Ai IR, which is a human-readable text
    /// format. Unlike conversion to an `Interpreter`, this does not consume the registered
    /// callables and properties, only consuming the program.
    ///
    /// If the most recent compilation errored, this will throw an exception.
    fn into_raw(&mut self) -> PyResult<String> {
        if self.program.is_none() {
            return Err(AiError::new_err("No program to convert"));
        }

        let mut program = String::new();
        for op in self.program.take().unwrap() {
            program += &(op.to_string() + "\n");
        }
        Ok(program)
    }
}

// #[pyclass]
// enum InterpreterState {
//     Yield,
//     Stop,
// }
// 
// impl From<AiInterpreterState> for InterpreterState {
//     fn from(state: AiInterpreterState) -> Self {
//         match state {
//             AiInterpreterState::Yield => Self::Yield,
//             AiInterpreterState::Stop => Self::Stop,
//         }
//     }
// }

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

    /// Checks if a value is a subclass of `CallableGenerator`. 
    ///
    /// This exists primarily as a convenience function for the implementation, and should 
    /// be treated as deprecated, though it is unlikely to ever be removed. 
    #[staticmethod]
    fn check_if_callable(value: &Bound<'_, PyAny>) -> bool {
        value.is_instance_of::<AiCallableGenerator>()
    }
    
    /// Registers the given `CallableGenerator` under the provided name. If the value is not a
    /// subclass of `CallableGenerator`, an exception is thrown.
    ///
    /// In order for a name to be valid in a given program, it must be registered with an
    /// associated `CallableGenerator`. This generator is then invoked any time the name is
    /// referenced as a call. For more details on those mechanics, refer to `CallableGenerator`.
    fn register_callable(&mut self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !Self::check_if_callable(&value) {
            return Err(AiError::new_err("Not an instance of ailangpy.CallableGenerator"));
        }

        map_pyerr!(self.0.register_callable(name, Box::new(PyCallableGenerator(value.unbind()))))
    }
    
    /// Checks if a value is a subclass of `Prop`. 
    ///
    /// This exists primarily as a convenience function for the implementation, and should 
    /// be treated as deprecated, though it is unlikely to ever be removed. 
    #[staticmethod]
    fn check_if_prop(value: &Bound<'_, PyAny>) -> bool {
        value.is_instance_of::<AiProp>()
    }
    
    /// Registers the given `Prop` under the provided name. If the value is not a
    /// subclass of `Prop`, an exception is thrown. Note that the given name should not be preceded
    /// by a dollar sign ($ - U+0024), as that is handled internally by the compiler.
    ///
    /// In order for a name to be valid in a given program, it must be registered with an
    /// associated `Prop`. This instance's methods are then used any time that name 
    /// is referenced in the program. For more details on those mechanics, refer to `Prop`.
    fn register_property(&mut self, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !Self::check_if_prop(&value) {
            return Err(AiError::new_err("Not an instance of ailangpy.Prop"));
        }

        map_pyerr!(self.0.register_property(name, Box::new(PyProp(value.unbind()))))
    }

    /// Resets the interpreter to its initial state. Does not discard any registered callables or
    /// properties. Does not invoke `terminate` for any active `Callable`s, so if that is required,
    /// call `stop` first.
    fn reset(&mut self) -> PyResult<()> {
        map_pyerr!(self.0.reset())
    }

    /// Stops execution of the interpreter, which invokes the `terminate` method of any active
    /// `Callable`s, as well as the `__end` group, if it is defined.
    fn stop(&mut self) -> PyResult<()> {
        map_pyerr!(self.0.end())
    }

    /// Invokes the interpreter, which will continue until it yields. Returns `True` if the program
    /// is complete, otherwise returns `False`.
    fn run(&mut self) -> PyResult<bool> {
        map_pyerr!(self.0.step().map(|s| s == AiInterpreterState::Yield))
    }
}


/// A Python implementation of the Ai automation language. For a specification of the language, as
/// well as an introduction to the external concepts, read the TODO docs.
#[pymodule(name = "ailangpy")]
fn load_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    // m.add_function(wrap_pyfunction!(print_value, m)?)?;

    m.add_class::<Interpreter>()?;
    m.add_class::<Compiler>()?;
    // m.add_class::<InterpreterState>()?;
    m.add_class::<AiArg>()?;
    m.add_class::<AiCallable>()?;
    m.add_class::<AiCallableGenerator>()?;
    m.add_class::<AiProp>()?;
    Ok(())
}
