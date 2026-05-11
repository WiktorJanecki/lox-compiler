use inkwell::context::Context;
use loxc::ast::Ast;
use loxc::codegen::codegen;

mod mock_print;
mod expr;
mod print;

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