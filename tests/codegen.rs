use inkwell::context::Context;
use inkwell::OptimizationLevel;
use loxc::ast::Ast;
use loxc::codegen::codegen;
use loxc::parser::parse;

#[test]
fn empty(){
    let ast = Ast::new();
    let mut context = Context::create();
    let module = codegen(ast, &mut context);

    assert!(module.is_ok());
}

#[test]
fn expr() -> anyhow::Result<()>{
    let ast = parse("print 2+2;")?;

    let mut context = Context::create();
    let module = codegen(ast, &mut context)?;
    // TODO: add_global_mapping -> printf function
    // that adds lines to global buffer
    let engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
    let args = vec![];
    unsafe {
        let r = engine.run_function_as_main(module.get_function("main").unwrap(), &args);
        assert_eq!(r, 0);
        // assert global print buffer == 4
        Ok(())
    }
}