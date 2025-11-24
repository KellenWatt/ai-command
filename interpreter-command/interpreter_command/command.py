import ailangpy
import commands2
from . import adapter

from typing import Optional

class AiCommand(commands2.Command):
    """
    Command for executing an Ai program based on the provided interpreter. This is just 
    a thin wrapper for the Interpreter, so all functionality, including loading programs
    and registering `Callable`s and `Prop`s should go through that interpreter.

    For information on Ai, refer to the [Ai Project](https://github.com/KellenWatt/ai-command).
    """
    terp: ailangpy.Interpreter
    complete: bool

    def __init__(self, interpreter: ailangpy.Interpreter, *subsystems: commands2.Subsystem):
        super().__init__()
        self.terp = interpreter
        self.subsystems = subsystems
        self.addRequirements(*subsystems)

    def initialize(self):
        self.complete = False
        #print("initializing AiCommand")
        for sub in self.subsystems:
            cmd = sub.getDefaultCommand()
            if cmd is not None:
                cmd.initialize()

        self.terp.reset()

    def execute(self):
        running = self.terp.run()
        for sub in self.subsystems:
            cmd = sub.getDefaultCommand()
            # These methods are guaranteed to be here, since we import adapter above
            if cmd is not None and not sub.has_been_marked():
                cmd.execute()
            sub.reset_usage_mark()
        if not running:
            self.complete = True

    def end(self, interrupted: bool):
        #print("ending AiCommand" + (" abruptly" if interrupted else ""))
        if interrupted:
            # otherwise (the equivalent of) stop will be called organically in "execute"
            self.terp.stop()
        for sub in self.subsystems:
            cmd = sub.getDefaultCommand()
            if cmd is not None:
                cmd.end(interrupted)

    def isFinished(self) -> bool:
        return self.complete


def _wait_checker(args: list[ailangpy.Arg]):
    assert len(args) == 1, "'wait' only accepts a single argument"
    assert args[0].is_value(), "'wait' only accepts a value, not a word"

def _print_checker(args: list[ailangpy.Arg]):
    for (i, arg) in enumerate(args):
        assert arg.is_value(), f"'print' does not accept words (word at {i})"

def interpreter_from_ir(ir: str) -> ailangpy.Interpreter:
    """
    Helper function to construct a basic Interpreter from pre-existing Ai IR, with built-in
    support for existing `commands2` commands.

    Two Callables are provided by default: `wait` and `print`, which respectively wrap
    `commands2.WaitCommand` and `commands2.PrintCommand`. Anything else must be 
    registered with the Interpreter manually. If you do not want these by default, or 
    want them to have different behaviour for those words, you should create an Interpreter
    directly.
    """
    terp = ailangpy.Interpreter(ir)
    terp.register_command("wait", commands2.WaitCommand, _wait_checker)
    terp.register_command("print", commands2.PrintCommand, _print_checker)
    return terp

