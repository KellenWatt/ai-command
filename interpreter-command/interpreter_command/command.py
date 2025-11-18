import ailangpy
import commands2
from . import adapter

class AiCommand(commands2.Command):
    terp: ailangpy.Interpreter
    complete: bool

    def __init__(self, interpreter: ailangpy.Interpreter):
        super().__init__()
        self.terp = interpreter
        self.complete = False

    def initialize(self):
        self.terp.reset()

    def execute(self):
        if not self.terp.run():
            self.complete = True

    def end(self, interrupted: bool):
        if interrupted:
            # otherwise (the equivalent of) stop will be called organically in "execute"
            self.terp.stop()

    def is_finished(self) -> bool:
        return self.complete


def _wait_checker(args: list[ailangpy.Arg]):
    assert len(args) == 1, "'wait' only accepts a single argument"
    assert args[0].is_value(), "'wait' only accepts a value, not a word"

def _print_checker(args: list[ailangpy.Arg]):
    for (i, arg) in enumerate(args):
        assert arg.is_value(), f"'print' does not accept words (word at {i})"

def interpreter_from_ir(ir: str) -> ailangpy.Interpreter:
    terp = ailangpy.Interpreter(ir)
    terp.register_command("wait", commands2.WaitCommand, _wait_checker)
    terp.register_command("print", commands2.PrintCommand, _print_checker)
    return terp

