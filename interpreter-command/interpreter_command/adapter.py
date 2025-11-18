import ailangpy
import commands2

from enum import Enum, auto
from typing import Callable, Any




def define_syntax(syntax: str) -> Callable[[list[ailangpy.Arg]], None]:
    """
    Generates a syntax checker for the specified syntax using a simple description format.
    Arguments that should be values are specified by a single literal asterisk '*' (without the quotes). 
    Anything else is treated as a word and is matched against exactly.

    Example:
    Given a hypothetical "turn" command that turns somthing to a given angle, the syntax string could be 
    'to * degrees' which would match the inputs 'to 90 degrees', 'to 1 degrees', 'to "foo" degrees'. 
    The middle token can match any value (even ones that don't make sense) since it's an asterisk, but the
    others have to match their corresponding parameter exactly.

    Note in the above example that the syntax string always matches 'degrees', not 'degree', only supports
    degrees, not radians, and can't describe a tolerance. This helper has no concept of pluralization, alternate 
    spellings, or multiple interpretations. To support these, you'll need to define your own syntax 
    checker manually (not terribly difficult, but supporting these behaviours in the general case would 
    require a non-trivial description language).
    """
    parts = syntax.split()
    params = [None if part == "*" else part for part in parts]

    def check(args: list[ailangpy.Arg]):
        assert len(params) == len(args), f"Incorrect argument count (Expected {len(params)}, got {len(args)})"
        for (i, (param, arg)) in enumerate(zip(params, args)):
            if arg.is_value():
                assert param is not None, f"Expected argument {i+1} to be a value"
            else: # word
                assert arg.matches_word(param), f"Expected argument {i+1} to be the word '{param}'"

    return check

def no_args() -> Callable[[list[ailangpy.Arg]], None]:
    """Generates a syntax checker that checks that no arguments were passed of any kind."""
    def check(args: list[ailangpy.Arg]):
        assert len(args) == 0
    return check

def simple_args(arg_count: int) -> Callable[[list[ailangpy.Arg]], None]:
    """
    Generates a syntax checker that simply checks if there are `arg_count` arguments and that they are all values.
    Fails if any arguments that are words.
    """
    def check(args: list[ailangpy.Arg]):
        value_args = len([arg for arg in args if arg.is_value()])
        assert len(args) == value_args, "Expected only value arguments but found a word"
        assert len(args) == arg_count, f"Expected {arg_count} arguments, got {len(args)}"

    return check


class CommandAdapter(ailangpy.CallableGenerator):
    """
    Interface for using a WPILib `Command` as an Ai `Callable`. This class implements `CallableGenerator`, and 
    generates the internal class `CommandAdapter.GeneratedCommandAdapter`, which implements `Callable` using 
    the wrapped `Command`'s lifecycle methods.

    Since `Command`s don't have the concept of syntax, and thus have nothing to check, you'll need to provide 
    a function (or Python `Callable` of any variety), that can be used for `CallableGenerator.check_syntax`.
    This is handled in the constructor.

    You can additionally supply some default arguments that will be passed through to every `Command` instance
    generated.
    """
    command_class: type[commands2.Command]
    checker: Callable[[list[ailangpy.Arg]], None]
    default_args: list[Any]

    class CommandState(Enum):
        """Internal type that represents the current state of a `GeneratedCommandAdapter`. You should never
        need to use this directly."""
        Inactive = auto()
        Active = auto()
        Complete = auto()

    class GeneratedCommandAdapter(ailangpy.Callable):
        """Internal type that adapts a `Command` into a `Callable`, using the `Command`'s lifecycle methods.
        You should never create an instance of this yourself, since it is only meant to be used inside the Ai
        Interpreter."""
        command: commands2.Command
        _state: "CommandAdapter.CommandState"

        def __init__(self, command: commands2.Command):
            self.command = command
            self._state = CommandAdapter.CommandState.Inactive

        def call(self) -> bool:
            if self._state == CommandAdapter.CommandState.Inactive:
                self.command.initialize()
                self._state = CommandAdapter.CommandState.Active
            self.command.execute()
            if self.command.isFinished():
                self.command.end(False)
                self._state = CommandAdapter.CommandState.Complete
                return True
            return False

        def terminate(self):
            if self._state == CommandAdapter.CommandState.Active:
                self.command.end(True)
                self._state = CommandAdapter.CommandState.Complete


    def __init__(self, command: type[commands2.Command], checker: Callable[[list[ailangpy.Arg]], None], default_args: list[Any] = []):
        """
        Creates a new instance of `CommandAdapter`, which wraps the given command type, and uses the given checker as the 
        `check_syntax` method. 

        If you specify any `default_args`, they are used as the first arguments to the `Command`'s
        constructor, before the arguments provided by Ai. This is useful for specifying fixed requirements, such as 
        resources like `Subsystem`s or controllers.
        """
        self.command_class = command
        self.checker = checker
        self.default_args = default_args

    def generate(self, args: list[Any]) -> "CommandAdapter.GeneratedCommandAdapter":
        cmd = self.command_class(*self.default_args, *args)
        return CommandAdapter.GeneratedCommandAdapter(cmd)

    def check_syntax(self, args: list[ailangpy.Arg]):
        self.checker(args)

def register_command(self: ailangpy.Interpreter, name: str, command: type[commands2.Command], checker: Callable[[list[ailangpy.Arg]], None], default_args: list[Any] = []):
    """
    Registers a Command with the given Interpreter instance, automatically wrapping it in the adapter. This method is
    monkey-patched into the actual Interpreter class, so there's no need to import it directly, so long as you import
    the interpreter_command module
    """
    adapter = CommandAdapter(command, checker, default_args)
    self.register_callable(name, adapter)


ailangpy.Interpreter.register_command = register_command

