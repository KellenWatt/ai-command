import ailangpy
import commands2

from enum import Enum, auto
from typing import Callable, Any


class CommandState(Enum):
    Inactive = auto()
    Active = auto()
    Complete = auto()


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

    Note in the above example that the syntax string always matches 'degrees', but not 'degree', only supports
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
    command_class: type[commands2.Command]
    checker: Callable[[list[ailangpy.Arg]], None]
    default_args: list[Any]

    class GeneratedCommandAdapter(ailangpy.Callable):
        command: commands2.Command
        _state: CommandState

        def __init__(self, command: commands2.Command):
            self.command = command
            self._state = CommandState.Inactive

        def call(self) -> bool:
            if self._state == CommandState.Inactive:
                self.command.initialize()
                self._state = CommandState.Active
            self.command.execute()
            if self.command.isFinished():
                self.command.end(False)
                self._state = CommandState.Complete
                return True
            return False

        def terminate(self):
            if self._state == CommandState.Active:
                self.command.end(True)
                self._state = CommandState.Complete


    def __init__(self, command: type[commands2.Command], checker: Callable[[list[ailangpy.Arg]], None], default_args: list[Any] = []):
        self.command_class = command
        self.checker = checker
        self.default_args = default_args

    def generate(self, args: list[int|float|str|bool|None]) -> "CommandAdapter.GeneratedCommandAdapter":
        cmd = self.command_class(*self.default_args, *args)
        return CommandAdapter.GeneratedCommandAdapter(cmd)

    def check_syntax(self, args: list[ailangpy.Arg]):
        self.checker(args)

def register_command(self: ailangpy.Interpreter, name: str, command: type[commands2.Command], checker: Callable[[list[ailangpy.Arg]], None], default_args: list[Any] = []):
    adapter = CommandAdapter(command, checker, default_args)
    self.register_callable(name, adapter)


ailangpy.Interpreter.register_command = register_command

