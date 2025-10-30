mod lexer;
mod token;
mod parser;
mod ast;
mod error;
mod compiler;
mod interpreter;

use itertools::Itertools;

use crate::lexer::{Lexer};
use crate::ast::{ExprVisitor, StmtVisitorMut, Arg as AstArg};
use crate::parser::{Parser};
use crate::compiler::{Compiler, Callable, CallableGenerator, Prop, Arg, Value};
use crate::error::Error;
use crate::interpreter::{Interpreter, InterpreterState};

pub struct AstPrinter {
    indent: usize,
}

// pub trait ExprVisitor<'a, R> {
//     fn visit_binary_expr(&self, expr: &Binary<'a>) -> R;
//     fn visit_grouping_expr(&self, expr: &Grouping<'a>) -> R;
//     fn visit_literal_expr(&self, expr: &Literal<'a>) -> R;
//     fn visit_logical_expr(&self, expr: &Logical<'a>) -> R;
//     fn visit_unary_expr(&self, expr: &Unary<'a>) -> R;
//     fn visit_variable_expr(&self, expr: &Variable<'a>) -> R;
// }

impl<'a> AstPrinter {
    // fn print(e: &ast::Stmt) {
    //     let mut printer = AstPrinter {indent: 0};
    //     let s: String = e.accept_mut(&mut printer);
    //     println!("{}", s);
    // }

    fn parenthesize(&self, name: &str, args: &[&ast::Expr]) -> String {
        format!("({} {})", name, args.into_iter().map(|exp| {
            exp.accept(self)
        }).join(" "))
    }
    fn indent(&self) -> String {
        " ".repeat(self.indent * 2)
    }
}


impl<'a> ExprVisitor<'a, String> for AstPrinter {
    fn visit_binary_expr(&self, expr: &ast::Binary<'a>) -> String {
        format!("{}", self.parenthesize(&expr.op.lexeme, &[&expr.left, &expr.right]))
    }

    fn visit_grouping_expr(&self, expr: &ast::Grouping<'a>) -> String {
        format!("{}", self.parenthesize(if expr.abs {"abs"} else {"group"}, &[&expr.expression]))
    }

    fn visit_literal_expr(&self, expr: &ast::Literal<'a>) -> String {
        format!("{}", expr.value)
    }

    fn visit_logical_expr(&self, expr: &ast::Logical<'a>) -> String {
        format!("{}", self.parenthesize(&expr.op.lexeme, &[&expr.left, &expr.right]))
    }

    fn visit_unary_expr(&self, expr: &ast::Unary<'a>) -> String {
        format!("{}", self.parenthesize(&expr.op.lexeme, &[&expr.right]))
    }

    fn visit_variable_expr(&self, expr: &ast::Variable<'a>) -> String {
        format!("{}", expr.name.lexeme)
    }
}
// pub trait StmtVisitorMut<'a, R> {
//     fn visit_group_stmt(&mut self, stmt: &Group<'a>) -> R;
//     fn visit_use_stmt(&mut self, stmt: &Use<'a>) -> R;
//     fn visit_if_stmt(&mut self, stmt: &If<'a>) -> R;
//     fn visit_while_stmt(&mut self, stmt: &While<'a>) -> R;
//     fn visit_exec_stmt(&mut self, stmt: &Exec<'a>) -> R;
//     fn visit_var_stmt(&mut self, stmt: &Var<'a>) -> R;
// }      
impl<'a> StmtVisitorMut<'a, String> for AstPrinter {
    fn visit_group_stmt(&mut self, stmt: &ast::Group<'a>) -> String {
        let param_list = stmt.params.iter().map(|t| t.lexeme).join(" ");
        self.indent += 1;
        let body = stmt.statements.iter().map(|s| {
            format!("{}{}", self.indent(), s.accept_mut(self))
        }).join("\n");
        self.indent -= 1;
        format!("(group {} {}\n{})", stmt.name.lexeme, param_list, body)
    }

    fn visit_use_stmt(&mut self, stmt: &ast::Use<'a>) -> String {
        format!("(use {})", stmt.name.lexeme)
    }

    fn visit_if_stmt(&mut self, stmt: &ast::If<'a>) -> String {
        let keyword = if stmt.invert {"unless"} else {"if"};
        let condition = stmt.condition.accept(self);
        self.indent += 1;
        let then_body = stmt.then_branch.iter().map(|s| {
            format!("{}{}", self.indent(), s.accept_mut(self))
        }).join("\n");
        self.indent -= 1;
        if stmt.else_branch.len() == 0 {
            format!("({} {} {})", keyword, condition, then_body)
        } else {
            self.indent += 1;
            let else_body = stmt.else_branch.iter().map(|s| {
                format!("{}{}", self.indent(), s.accept_mut(self))
            }).join("\n");
            self.indent -= 1;
            format!("({} {} \n{}\n{})", keyword, condition, then_body, else_body)
        }
    }
    fn visit_while_stmt(&mut self, stmt: &ast::While<'a>) -> String {
        let keyword = if stmt.invert {"until"} else {"while"};
        let condition = stmt.condition.accept(self);
        self.indent += 1;
        let body = stmt.body.iter().map(|s| {
            format!("{}{}", self.indent(), s.accept_mut(self))
        }).join("\n");
        self.indent -= 1;

        format!("({} {}\n{})", keyword, condition, body)
    }
    fn visit_exec_stmt(&mut self, stmt: &ast::Exec<'a>) -> String {
        let arg_list = stmt.args.iter().map(|arg| {
            match arg {
                AstArg::Word(t) => t.lexeme.to_string(),
                AstArg::Value(e) => e.accept(self),
            }
        }).join(" ");
        format!("(call {} {})", stmt.name.lexeme, arg_list)
    }
    fn visit_var_stmt(&mut self, stmt: &ast::Var<'a>) -> String {
        format!("(set {} {})", stmt.name.lexeme, stmt.value.accept(self))
    }

    fn visit_parallel_stmt(&mut self, stmt: &ast::Parallel<'a>) -> String {
        let calls = stmt.calls.iter().map(|call| {
            format!("({})", self.visit_exec_stmt(call))
        }).join(" ");

        let kind = if stmt.race {"race"} else {"parallel"};
        format!("({} {})", kind, calls)
    }
}


struct Print;

impl CallableGenerator for Print {
    fn generate(&mut self) -> Box<dyn Callable> {
        Box::new(Print)
    }

    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error> {
        if args.len() != 1 || !args[0].is_value() {
            return Err(Error::Call("Expected a value as the only argument".into()));
        }
        Ok(())
    }
}

impl Callable for Print {
    fn call(&mut self, args: Vec<Value>) -> bool {
        println!("{}", args[0]);
        true
    }
}

struct CountdownGen;

impl CallableGenerator for CountdownGen {
    fn generate(&mut self) -> Box<dyn Callable> {
        Box::new(Countdown{count: 10})
    }
    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error> {
        if args.len() > 0 {
            return Err(Error::Call("Expected no arguments".into()));
        }
        Ok(())
    }
}

struct Countdown {
    count: u32,
}

impl Callable for Countdown {
    fn call(&mut self, _args: Vec<Value>) -> bool {
        if self.count == 0 {
            println!("liftoff!");
            return true;
        }
        println!("{}", self.count);
        self.count -= 1;
        false
    }

    fn terminate(&mut self) {
        println!("launch aborted at {}!", self.count);
    }

}

struct CountupGen {
    max: u32,
}

impl CallableGenerator for CountupGen {
    fn generate(&mut self) -> Box<dyn Callable> {
        Box::new(Countup::new(self.max))
    }

    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error> {
        if args.len() > 0 {
            return Err(Error::Call("Expected no arguments".into()));
        }
        Ok(())
    }
}

struct Countup {
    count: u32,
    max: u32,
}
impl Countup {
    fn new(max: u32) -> Countup {
        Countup {
            count: 0,
            max,
        }
    }
}

impl Callable for Countup {
    fn call(&mut self, _args: Vec<Value>) -> bool {
        if self.count == self.max {
            println!("Ready or not!");
            return true;
        }
        println!("{}", self.count + 1);
        self.count += 1;
        false
    }

    fn terminate(&mut self) {
        println!("I give up!");
    }

}

struct DummyProp;
impl Prop for DummyProp {
    fn get(&self) -> Value {
        Value::Number(42.0)
    }
    fn set(&mut self, v: Value) {
        println!("set {}", v);
    } 
    fn settable(&self) -> bool {true}
}

struct TimeProp;
impl Prop for TimeProp {
    fn get(&self) -> Value {
        Value::Number(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64())
    }
}

fn main() {
    let source = 
        "
        group __end {
            print 'cleaning up!';
        }

        group off_by_one {
            print 'delay';
            countdown;
        }
        race {
            off_by_one;
            countdown;
            countup;
        }
        print 'Hello!';";
        // "use $time;
        // group greet $name {
        //     print 'Hello, ' + $name + '!';
        // }
        // 
        // group repeat $value $times times {
        //     $i = 0;
        //     until $i >= $times {
        //         $i += 1;
        //         print $value;
        //     }
        // }
        // 
        // group convention $name {
        //     race {
        //         repeat 'echo' 3 times;
        //         countdown;
        //         countup;
        //     }
        // }
        // $start = $time;
        // print $start;
        // greet 'World';
        // convention 'Me';
        // print $time - $start;";

        // "
        // print \"Hello, World!\";
        // greet 'You';
        // print \"Goodbye, Mars!\";
        // convention \"Me\";";
        // "use $angle;
        // use $position;
        // sequence group go_right $unit {
        //     $var = 1;
        //     right 90 degrees;
        //     forward $unit;
        // }
        // while $angle < 270 {
        //     go_right 1;
        // }
        // ";
    //     "use $alliance;
    //     if $alliance == 'Blue' {
    //         go left 10;
    //     } else if $alliance == 'Red' {
    //         go right 10;
    //     } else {
    //         do nothing;
    //     }
    //     ";
    let lex = Lexer::new(source);
    

    // for tok in lex {
    //     println!("{}", tok);
    // }
    // return;
    
    let mut parser = Parser::new(lex);
    let ast = parser.parse();
    // if let Some(ref ast) = ast {
    //     for stmt in ast.iter() {
    //         AstPrinter::print(&stmt);
    //     }
    // }
    let mut compiler = Compiler::new();
    let _ = compiler.register_callable("print", Box::new(Print));
    let _ = compiler.register_callable("countdown", Box::new(CountdownGen));
    let _ = compiler.register_callable("countup", Box::new(CountupGen{max: 5}));
    let _ = compiler.register_property("angle", Box::new(DummyProp));
    let _ = compiler.register_property("position", Box::new(DummyProp));
    let _ = compiler.register_property("time", Box::new(TimeProp));
    if let Some(ast) = ast {
        match compiler.compile(ast) {
            Ok(program) => {
                // let magnitude = program.code.len().ilog10() as usize + 1;
                // for (i, line) in program.code.iter().enumerate() {
                //     println!("{:>magnitude$} {}", i, line);
                // }
                let mut terp = Interpreter::from_program(program);
                loop {
                    match terp.step() {
                        Ok(InterpreterState::Stop) => {
                            break;
                        }
                        Ok(InterpreterState::Yield) => {
                            // println!("yielding");
                            continue;
                        }
                        Err(e) => {
                            eprintln!("{}", e);
                            break;
                        }
                    }
                }
                // while terp.step() != ExecutionState::Stop {}
            }
            Err(err) => {
                for e in err {
                    eprintln!("{}", e);
                }
            }
        }
    } else {
        for error in parser.errors {
            println!("{}", error);
        }
    }
}
