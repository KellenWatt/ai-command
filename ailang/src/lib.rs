mod lexer;
mod token;
mod parser;
mod ast;
mod error;
mod compiler;
mod interpreter;

pub use crate::lexer::{Lexer};
pub use crate::parser::{Parser};
pub use crate::error::{Error, Result};
pub use crate::compiler::{Compiler, Callable, CallableGenerator, Prop, Arg, Value, Program, Op};
pub use crate::interpreter::{Interpreter as AiInterpreter, InterpreterState};



pub struct AiCompiler {
    compiler: Option<Compiler>,
}

impl AiCompiler {

    pub fn new() -> AiCompiler {
        AiCompiler {
            compiler: None,
        }
    }
    
    pub fn register_callable<C: CallableGenerator + 'static>(&mut self, name: &str, callable: C) -> Result<()> {
        if self.compiler.is_none() {
            self.compiler = Some(Compiler::new());
        }
        self.compiler.as_mut().unwrap().register_callable(name, Box::new(callable))
    }
    
    pub fn register_property<P: Prop + 'static>(&mut self, name: &str, prop: P) -> Result<()> {
        if self.compiler.is_none() {
            self.compiler = Some(Compiler::new());
        }
        self.compiler.as_mut().unwrap().register_property(name, Box::new(prop))
    }

    pub fn compile(&mut self, source: &str) -> std::result::Result<Program, Vec<Error>> {
        let lexer = Lexer::new(source);
        
        let mut parser = Parser::new(lexer);
        let ast = parser.parse();
        if ast.is_none() {
            return Err(parser.errors);
        }
        let ast = ast.unwrap();
        let compiler = self.compiler.take().unwrap_or_else(|| Compiler::new());
        compiler.compile(ast)
    }

    pub fn compile_nonconsuming(&mut self, source: &str) -> std::result::Result<Vec<Op>, Vec<Error>> {
        let lexer = Lexer::new(source);
        
        let mut parser = Parser::new(lexer);
        let ast = parser.parse();
        if ast.is_none() {
            return Err(parser.errors);
        }
        let ast = ast.unwrap();
        if self.compiler.is_none() {
            self.compiler = Some(Compiler::new());
        }
        self.compiler.as_mut().unwrap().compile_nonconsuming(ast)
    }

    pub fn package_program(&mut self, code: Vec<Op>) -> Program {
        if self.compiler.is_none() {
            self.compiler = Some(Compiler::new());
        }
        self.compiler.as_mut().unwrap().package_program(code)
    }

    pub fn convert(&mut self, source: &str) -> std::result::Result<AiInterpreter, Vec<Error>> {
        let program = self.compile(source)?;
        Ok(AiInterpreter::from_program(program))
    }
}

