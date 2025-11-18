from typing import Union, Any, final, Annotated

Value = Annotated[Union[str, int, float, bool, None],
    """
    A generic Ai value.

    WARNING: This type is not actually defined or exported in any way by ailangpy. 
    This is strictly for documentation convenience. Any attempt to use this as a
    type hint *will* cause things to blow up in exciting ways.

    Instead, prefer `typing.Any` or define your own copy. Both should be close enough
    in a vast majority of cases.
    """
    ]

@final
class Interpreter:
    @staticmethod
    def check_if_callable(value: Any) -> bool:
        """
        Checks if a value is a subclass of `CallableGenerator`. 
        
        This exists primarily as a convenience function for the implementation, and should 
        be treated as deprecated, though it is unlikely to ever be removed. 
        """
        ...
    @staticmethod
    def check_if_prop(value: Any) -> bool:
        """
        Checks if a value is a subclass of `Prop`. 
        
        This exists primarily as a convenience function for the implementation, and should 
        be treated as deprecated, though it is unlikely to ever be removed. 
        """
        ...
    def register_callable(self, name: str, value: CallableGenerator) -> None:
        """
        Registers the given `CallableGenerator` under the provided name. If the value is not a
        subclass of `CallableGenerator`, an exception is thrown.
        
        In order for a name to be valid in a given program, it must be registered with an
        associated `CallableGenerator`. This generator is then invoked any time the name is
        referenced as a call. For more details on those mechanics, refer to `CallableGenerator`.
        """
        ...
    def register_property(self, name: str, value: Prop) -> None:
        """
        Registers the given `Prop` under the provided name. If the value is not a
        subclass of `Prop`, an exception is thrown. Note that the given name should not be preceded
        by a dollar sign ($ - U+0024), as that is handled internally by the compiler.
        
        In order for a name to be valid in a given program, it must be registered with an
        associated `Prop`. This instance's methods are then used any time that name 
        is referenced in the program. For more details on those mechanics, refer to `Prop`.
        """
        ...

    def run(self) -> bool:
        """
        Invokes the interpreter, which will continue until it yields. Returns `True` if the program
        is complete, otherwise returns `False`.
        """
        ...
    def stop(self) -> None:
        """
        Stops execution of the interpreter, which invokes the `terminate` method of any active
        `Callable`s, as well as the `__end` group, if it is defined.
        """
        ...
    def reset(self) -> None:
        """
        Resets the interpreter to its initial state. Does not discard any registered callables or
        properties. Does not invoke `terminate` for any active `Callable`s, so if that is required,
        call `stop` first.
        """
        ...

@final
class Compiler:
    """
    A compiler for the Ai language. Performs syntax checking and name resolution, storing a
    text-based intermediate representation (IR) that is executed by the Ai interpreter. A
    `Compiler` can be converted directly to an `Interpreter` using the `into_interpreter` method, or it
    can be converted into the raw IR using the `into_raw` method.

    Note that in order to use custom `Callable`s and `Prop`s in a program, you must register them
    with the compiler so that it can recognize them in a program. For more details , see the
    respective `register_*` methods.
    """
    @staticmethod
    def check_if_callable(value: Any) -> bool:
        """
        Checks if a value is a subclass of `CallableGenerator`. 
        
        This exists primarily as a convenience function for the implementation, and should 
        be treated as deprecated, though it is unlikely to ever be removed. 
        """
        ...
    @staticmethod
    def check_if_prop(value: Any) -> bool: 
        """
        Checks if a value is a subclass of `Prop`. 
        
        This exists primarily as a convenience function for the implementation, and should 
        be treated as deprecated, though it is unlikely to ever be removed. 
        """
        ...
    def register_callable(self, name: str, value: CallableGenerator) -> None:
        """
        Registers the given `CallableGenerator` under the provided name. If the value is not a
        subclass of `CallableGenerator`, an exception is thrown.
        
        In order for a name to be valid in a given program, it must be registered with an
        associated `CallableGenerator`. This generator is then invoked any time the name is
        referenced as a call. For more details on those mechanics, refer to `CallableGenerator`.
        """
        ...
    def register_property(self, name: str, value: Prop) -> None:
        """
        Registers the given `Prop` under the provided name. If the value is not a
        subclass of `Prop`, an exception is thrown. Note that the given name should not be preceded
        by a dollar sign ($ - U+0024), as that is handled internally by the compiler.
        
        In order for a name to be valid in a given program, it must be registered with an
        associated `Prop`. This instance's methods are then used any time that name 
        is referenced in the program. For more details on those mechanics, refer to `Prop`.
        """
    def has_program(self) -> bool:
        """
        Checks if the compiler has been given a program and that it compiled successfully. Returns
        the same result as `compile`.
        """
        ...
    def into_raw(self) -> str:
        """
        Takes the most recent program and converts it to the Ai IR, which is a human-readable text
        format. Unlike conversion to an `Interpreter`, this does not consume the registered
        callables and properties, only consuming the program.
        
        If the most recent compilation errored, this will throw an exception.
        """
        ...
    def compile(self, source: str) -> bool:
        """
        Compiles the provided program, storing the results internally, returning `True` if the
        program was compiled successfully, and False if there was some form of compilation error.
        
        If a `compile` is called more than once, previous results, both program and errors, are discarded.
        
        If any errors are raised by the compiler, they are also stored, and can be viewed using 
        `print_errors`. At present, there is no way to inspect the errors programatically.
        """
        ...
    def print_errors(self) -> None:
        """
        Displays any errors that occured during the most recent compilation. Prints nothing if
        there are no errors.
        """
        ...
    def into_interpreter(self) -> Interpreter: 
        """
        Effectively consumes the compiler, converting it into an `Interpreter`. Throws an exception
        if there is no valid program stored. 
        
        In principle, on success, this method completely consumes the compiler, moving all of its assets,
        including the registered callables and properties, over to the new interpreter, disallowing
        further access to the compiler. In practice, because of Python's memory model, a successful call 
        effectively just removes the contents of the compiler, functionally resetting it to its
        initial state. This means you will need to re-register all callables and properties to
        re-compile the same program.
        """
        ...

@final
class Arg:
    """
    A class representing the compile-time concept of an argument. Used exclusively in
    `CallableGenerator.check_syntax`.

    An argument can be either a value or a word. A value represents an Ai runtime value, but it
    can not currently hold that value, since Ai values don't exist at compile-time. A value can be
    recognized using the `Arg.is_value` method. A word is anything that isn't a value, variable name, 
    or reserved syntactical element. This *is* known at compile-time, and can be matched against 
    using the `Arg.matches_word` method. If you only care about the presence of a word, you can use
    the `Arg.is_word` method.
    """
    def is_value(self) -> bool: 
        """
        Returns whether an Arg represents a value. The value itself is not known, just that it
        will eventually be *some* value.
        """
        ...
    def is_word(self) -> bool: 
        """
        Returns whether an Arg represents a word. Since words are known at compile-time,
        `Arg.matches_word` is more likely to be useful for checks.
        """
        ...
    def matches_word(self, s: str) -> bool: 
        """
        Returns whether both an Arg is a word, and that word matches the given string exactly. 
        
        Checking is done using a simple byte-wise equality, so diacritics and other non-unique
        Unicode may complicate things.
        """
        ...

class Callable:
    """A superclass required for defining an Ai-compatible native callable. 

    This is a virtual(-ish) class that does not have a valid implementation.

    Subclasses of this type should never need to be instantiated directly. Instead, you 
    should implement a `CallableGenerator` for this that creates instances of this class. 
    For more information on behaviour, please refer to `CallableGenerator`.

    Note that in some simple cases, it may be possible to make a class the subclass of both
    `Callable` and `CallableGenerator`. This is perfectly valid."""
    def call(self) -> bool: 
        """Called by the Ai runtime any time a native call associated with this type is active. A
        return value of True indicates the call is complete, while False causes the Ai interpreter to
        yield, calling this function again when execution resumes. A value of None is also valid
        and is treated the same as True (this simplifies writing simple `Callable`s).
        """
        ...
    def terminate(self) -> None: 
        """
        Called by the Ai runtime whenever a native call associated with this type is to be ended
        early. This can happen if a race group containing native calls ends, or if the Ai interpreter
        itself is ended early.
        
        Note: This is **not** called on a runtime error in Ai. If a runtime error occurs, cleanup must be
        performed outside the context of the interpreter.
        """
        ...

class CallableGenerator:
    """
    A superclass required for defining an Ai-compatible native callable.

    This is a virtual(-ish) class that does not have a valid implementation.

    The purpose of this class is to generate a new instance of each `Callable` whenever Ai
    needs to execute a new native call. At this time, a new `Callable` will be generated each time
    the native call is reached, (though not for each iteration of that native call). This is
    because it is entirely possible with parallel groups to have multiple instances of a given 
    callable running at once, even for the same instruction.

    If this is undesirable, the generator can be designed to return a singleton-style `Callable`.
    However, this is discouraged as it may cause issues with parallelism if the call is non-trivial.
    """
    def generate(self, args: list[Any]) -> Callable: 
        """
        Called by the Ai runtime when it encounters a native call associated with this type.
        Generates a new `Callable` based on the arguments passed from Ai.
        
        Most generators will simply pass the arguments to this function straight to the constructor
        for the associated `Callable`, however this method can be used to insert arguments (as in
        some kind of dependency injection), or filter them out, or even dispatch to multiple different
        kinds of `Callable`s.
        
        Note: unlike `check_syntax` these arguments will be actual values, and only values. Any
        syntactical words are filtered out by the Ai compiler.
        """
        ...
    
    def check_syntax(self, args: list[Arg]) -> None: 
        """
        Called by the Ai compiler to check if a call's syntax is considrered valid. It is
        considered valid if no exception was thrown. This allows for simple checking use Python's
        built-in `assert` to check type and count.

        For more information, check the documentation for the `Arg` type.

        Note: at this point, actual argument values are not known, just that there is a Value in
        that position. Words, however, only exist at compile-time, so their value is known and can
        be checked or even switched upon to provide a form of overloading (making this actually
        play nice at runtime is an exercise left to the user).
        """
        ...

class Prop:
    """
    A superclass required for defining an Ai property. This is a virtual(-ish) class that does 
    not have a valid implementation.

    A property is treated like a variable in the Ai runtime, but instead of getting or setting a
    variable of the associated name, the `get` and `set` methods of this type are called,
    respectively.

    Having a Prop be settable is entirely optional, since assigning to a source (for example, a
    sensor) doesn't always make sense. By default, a property is not settable, as determined by the
    value returned by the `settable` method. This can be changed by overriding it.

    If a property is not settable, the associated `Prop` does not need to define a `set` method, 
    since the compiler checks for settability, and the interpreter validates IR before it's run.

    Unlike `Callable`s, a property being accessed does not cause the Ai runtime to yield.
    """
    def get(self) -> Value: 
        """
        Retrieves a value from the property. The only allowed types are `int`, `float`, `str`,
        `bool` and `None`. Anything else will cause a runtime error.
        
        This method should never be called directly, as its return value is completely opaque and
        unusable in Python.
        """
        ...
    def set(self, value: Value) -> None: 
        """
        Sets the value of the property, for some definition of "set". Setting a property to a
        specific value does *not* imply that a subsequent call to `get` will return the same value.
        
        This method is only required if your program will set the value. There's no 
        syntactical way to prevent assignment, but an unassignable property will report errors on
        assignment at both compile-time and runtime.
        
        This method should never be called directly, as it is impossible to create the required
        value in Python.
        """
        ...
    def is_settable(self) -> bool: 
        """
        Returns whether the property is settable. If a property is not settable, the `set` method
        does not need to be implemented, as it can never be called.
        """
        ...
