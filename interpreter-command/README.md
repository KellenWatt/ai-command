# Ai Interpreter Command

This project allows the use of Ai-based commands in RobotPy programs for FRC.

## What is Ai?
Ai is a programming language that was designed to be extensible and, more importantly, interruptable. 
More accurately, Ai was designed as a sort of automation language, similar to `bash` or `make`. The
most important feature however, is the ability to interface directly with its outside environment in the
form of Callables and Props, which let the user define arbitrary interactions.

For more information, see the [Ai Project](https://github.com/KellenWatt/ai-command). (This package is 
included in the overall Ai Command project, since it was technically the driving force behind the whole thing,
and reorganizing repositories is a pain).

## What Happend to the Old `interpreter-command`
TL;DR: if you liked the old version, don't upgrade past 2024 versions.

The original version of this package was intended as a way to allow for on-the-fly changes to robot 
behaviour without having to go through the multi-second deploy and startup time, which quickly adds up over 
the course of developing, for example, a non-trivial autonomous routine. 

It was fine. It worked. It accomplished the goal. But it was clunky. It did nothing more than execute
a series of commands line-by-line, with a very simple ability to repeat or skip single commands using 
a highly inflexible syntax. It was less a language and more a glorified state machine. And if you
wanted to do anything in parallel? You had to do that natively, then register that parallelism under a 
single name, meaning it was completely inflexible.

Honestly, the only thing it had going for it was the dispatch system, which allowed you to define 
multiple commands that used the same initial name, then dispatching based on the second word, and so on.
That was (in my humble opinion as the author), kind of neat, especially since it was mostly an unintended 
side-effect, but it was a pain to set up.

Ai is different. Ai is a real, (*technically*) Turing-complete language, complete with compiler, intepreter, 
and support for transferring a pre-compiled program between environments (say, between a host device and 
a connected robot), in order to more efficiently distribute work. It's built in Rust, and embedded in Python,
so it's more efficient and doesn't just freeload off of Python's type system, which makes it much safer 
to use (albeit slightly less expressive). It also has a complete logic system that allows skipping arbitrary 
code, instead of one single command. Most importantly though, Ai has built-in support for parallel groups, 
which allows for more logic to be written within Ai, from more elemental components.

From version 2025 onward, this project will be backed by Ai, and bears no similarity to the old project,
except maybe a few shared names. The API is and will be completely different and will not be compatible 
with pre-2025 versions.
