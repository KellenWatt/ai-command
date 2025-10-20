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
use crate::compiler::{Compiler, Callable, Prop, Arg, Value};
use crate::error::Error;

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
    fn print(e: &ast::Stmt) {
        let mut printer = AstPrinter {indent: 0};
        let s: String = e.accept_mut(&mut printer);
        println!("{}", s);
    }

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
        format!("{}", self.parenthesize("group", &[&expr.expression]))
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
        let kind = match stmt.kind {
            ast::GroupKind::Sequence => "sequence",
            ast::GroupKind::Parallel => "parallel",
            ast::GroupKind::Race => "race",
        };
        let param_list = stmt.params.iter().map(|t| t.lexeme).join(" ");
        self.indent += 1;
        let body = stmt.statements.iter().map(|s| {
            format!("{}{}", self.indent(), s.accept_mut(self))
        }).join("\n");
        self.indent -= 1;
        format!("({}-group {} {}\n{})", kind, stmt.name.lexeme, param_list, body)
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
}

struct DummyCallable;

impl Callable for DummyCallable {
    fn call(&mut self) -> bool {
        true
    }

    fn check_syntax(&self, args: Vec<Arg>) -> Result<(), Error> {
        if !args[0].is_value() {
            return Err(Error::Call("Expected first arg to be a number".into()));
        }
        if args.len() == 2 && !args[1].get_word().map(|w| w == "degrees").unwrap_or(false) {
            return Err(Error::Call("Expected literal word 'degrees'".into()))
        }
        Ok(())
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

fn main() {
    let source = 
        "use $angle;
        use $position;
        sequence group go_right $unit {
            $var = 1;
            right 90 degrees;
            forward $unit;
        }
        while $angle < 270 {
            go_right 1;
        }
        ";
    //     "use $alliance;
    //     if $alliance == 'Blue' {
    //         go left 10;
    //     } else if $alliance == 'Red' {
    //         go right 10;
    //     } else {
    //         do nothing;
    //     }
    //     ";
    let mut lex = Lexer::new(source);
    

    // for tok in lex {
    //     println!("{}", tok);
    // }
    
    let mut parser = Parser::new(lex);
    let ast = parser.parse();
    if let Some(ref ast) = ast {
        for stmt in ast.iter() {
            AstPrinter::print(&stmt);
        }
    }
    let mut compiler = Compiler::new();
    let _ = compiler.register_callable("right", Box::new(DummyCallable));
    let _ = compiler.register_callable("forward", Box::new(DummyCallable));
    let _ = compiler.register_property("angle", Box::new(DummyProp));
    let _ = compiler.register_property("position", Box::new(DummyProp));
    if let Some(ast) = ast {
        match compiler.compile(ast) {
            Ok(program) => {
                for (i, line) in program.code.into_iter().enumerate() {
                    println!("{:>2} {}", i, line);
                }
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
