use inkwell::OptimizationLevel;
use inkwell::context::Context;
use loxc::codegen::codegen;
use loxc::parser::parse;
use std::cell::RefCell;
use std::ffi::c_char;

thread_local! {
    /// Theoretically thread-safe, so does not interrupt quick testing
    static PRINT_BUFFER: RefCell<String> = RefCell::new(String::new());
    static HAD_RUNTIME_ERROR: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Mock print linked against our generated llvm to assert program output
/// Done this way because print is the only possible side effect in lox
unsafe extern "C" fn mock_printf(_format: *const c_char, val: f64) -> i32 {
    PRINT_BUFFER.with(|buf| {
        use std::fmt::Write;
        write!(buf.borrow_mut(), "{}", val).ok();
    });
    0
}

/// Mock exit because libc exit kills test process
unsafe extern "C-unwind" fn mock_exit(_code: i32) -> ! {
    HAD_RUNTIME_ERROR.with(|f| f.set(true));
    // panic is catchable
    panic!("LoxRuntimeError");
}

/// Run program and assert print statement output. Asserts that program ran without runtime errors
pub fn assert_output(src: &'static str, should_output: &'static str) -> anyhow::Result<()> {
    PRINT_BUFFER.with(|buf| buf.borrow_mut().clear());
    let ast = parse(src)?;
    let mut context = Context::create();
    let module = codegen(ast, &mut context)?;

    let printf_fn = module
        .get_function("printf")
        .ok_or_else(|| anyhow::anyhow!("printf not declared in module"))?;
    let engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
    engine.add_global_mapping(&printf_fn, mock_printf as usize);

    unsafe {
        let r = engine.run_function_as_main(module.get_function("main").unwrap(), &[]);
        assert_eq!(r, 0);

        let output = PRINT_BUFFER.with(|buf| buf.borrow().clone());
        assert_eq!(output.trim(), should_output);
        Ok(())
    }
}

/// Asserts that program ends with runtime error
pub fn should_runtime_error(src: &'static str) -> anyhow::Result<()> {
    PRINT_BUFFER.with(|buf| buf.borrow_mut().clear());
    HAD_RUNTIME_ERROR.set(false);
    let ast = parse(src)?;
    let mut context = Context::create();
    let module = codegen(ast, &mut context)?;

    let engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
    let exit_fn = module.get_function("exit").unwrap();
    engine.add_global_mapping(&exit_fn, mock_exit as usize);

    unsafe {
        // catch all calls to libc exit
        let _= std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.run_function_as_main(module.get_function("main").unwrap(), &[]);
        }));
        // this will panic and kill the test process
        let had_err = HAD_RUNTIME_ERROR.get();
        assert!(had_err);
        Ok(())
    }
}
