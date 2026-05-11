use std::fmt::Write;
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use loxc::codegen::codegen;
use loxc::parser::parse;
use std::cell::RefCell;
use std::ffi::{c_char, CStr};

thread_local! {
    /// Theoretically thread-safe, so does not interrupt quick testing
    static PRINT_BUFFER: RefCell<String> = RefCell::new(String::new());
    static HAD_RUNTIME_ERROR: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// Mock print linked against our generated llvm to assert program output
/// Done this way because print is the only possible side effect in lox
unsafe extern "C" fn mock_printf(format: *const c_char, arg_int: usize, arg_float: f64) -> i32 {
    // both arguments are captured because of how variadics works
    // floats go to different registers as ints so even if first argument is float
    // arg float will capture it
    let fmt_str = CStr::from_ptr(format).to_string_lossy();

    PRINT_BUFFER.with(|buf| {
        let mut b = buf.borrow_mut();
        b.clear();
        if fmt_str.contains("%f") {
            write!(b, "{}", arg_float).ok();
        } else if fmt_str.contains("%s") {
            let s = CStr::from_ptr(arg_int as *const c_char).to_string_lossy();
            write!(b, "{}", s).ok();
        } else {
            write!(b, "{}", fmt_str.trim_end_matches('\n')).ok();
        }
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
    assert_eq!(run_with_print(src)?, should_output);

    Ok(())
}
pub fn assert_output_f64(src: &'static str, should_output: f64) -> anyhow::Result<()> {
    PRINT_BUFFER.with(|buf| buf.borrow_mut().clear());
    let number: f64 = run_with_print(src)?.parse()?;

    let epsilon = 0.0001;
    // eprintln!("{} {}", number, should_output);
    assert!((number - should_output).abs() < epsilon );
    Ok(())
}

fn run_with_print(src: &'static str) -> anyhow::Result<String> {

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

        let output = PRINT_BUFFER.with(|buf| buf.borrow().trim().to_string());
        Ok(output)
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
