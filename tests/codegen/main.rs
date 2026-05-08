use inkwell::context::Context;
use loxc::ast::Ast;
use loxc::codegen::codegen;
use crate::mock_print::{assert_output, should_runtime_error};

mod mock_print;

#[test]
fn empty() -> anyhow::Result<()> {
    // empty script should compile and have entrypoint

    let ast = Ast::new();
    let mut context = Context::create();
    let module = codegen(ast, &mut context)?;
    let main = module.get_function("main");

    // necessary for runtime errors and testing env
    let exit = module.get_function("exit");
    let printf = module.get_function("printf");
    assert!(main.is_some());
    assert!(exit.is_some());
    assert!(printf.is_some());

    Ok(())
}

#[test]
fn expr() -> anyhow::Result<()> {
    assert_output("print 2+2;", "4")?;
    assert_output("print 2+2*2;", "6")?;
    should_runtime_error("print true + true;")?;
    should_runtime_error("print 2.0 + true;")?;

    Ok(())
}

