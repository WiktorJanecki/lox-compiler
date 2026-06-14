use inkwell::OptimizationLevel;
use inkwell::context::Context;
use loxc::codegen::codegen;
use loxc::parser::parse;
use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::fmt::Write;

thread_local! {
    static PRINT_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
    static HAD_RUNTIME_ERROR: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

unsafe extern "C" fn mock_printf(format: *const c_char, arg_int: usize, arg_float: f64) -> i32 {
    unsafe {
        let fmt_str = CStr::from_ptr(format).to_string_lossy();

        PRINT_BUFFER.with(|buf| {
            let mut b = buf.borrow_mut();
            if fmt_str.contains("%f") {
                writeln!(b, "{}", arg_float).ok();
            } else if fmt_str.contains("%s") {
                let s = CStr::from_ptr(arg_int as *const c_char).to_string_lossy();
                writeln!(b, "{}", s).ok();
            } else {
                writeln!(b, "{}", fmt_str.trim_end_matches('\n')).ok();
            }
        });
        0
    }
}

unsafe extern "C-unwind" fn mock_exit(_code: i32) -> ! {
    HAD_RUNTIME_ERROR.set(true);
    panic!("LoxRuntimeError");
}

pub fn assert_output(src: &'static str, should_output: &'static str) -> anyhow::Result<()> {
    PRINT_BUFFER.with(|buf| buf.borrow_mut().clear());
    assert_eq!(run_with_print(src)?, should_output);
    Ok(())
}

pub fn assert_output_f64(src: &'static str, should_output: f64) -> anyhow::Result<()> {
    PRINT_BUFFER.with(|buf| buf.borrow_mut().clear());
    let output = run_with_print(src)?;
    let last_line = output.lines().last().unwrap_or(&output);
    let number: f64 = last_line.trim().parse()?;
    let epsilon = 0.0001;
    assert!((number - should_output).abs() < epsilon);
    Ok(())
}

unsafe extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn realloc(ptr: *mut u8, size: usize) -> *mut u8;
    fn strlen(s: *const u8) -> usize;
    fn strcpy(dst: *mut u8, src: *const u8) -> *mut u8;
    fn strcat(dst: *mut u8, src: *const u8) -> *mut u8;
    fn strcmp(s1: *const u8, s2: *const u8) -> i32;
}

fn add_c_lib_mappings(
    module: &inkwell::module::Module,
    engine: &inkwell::execution_engine::ExecutionEngine,
) {
    let fns = [
        ("malloc", malloc as *const ()),
        ("realloc", realloc as *const ()),
        ("strlen", strlen as *const ()),
        ("strcpy", strcpy as *const ()),
        ("strcat", strcat as *const ()),
        ("strcmp", strcmp as *const ()),
    ];
    for (name, ptr) in fns {
        if let Some(f) = module.get_function(name) {
            engine.add_global_mapping(&f, ptr as usize);
        }
    }
}

fn run_with_print(src: &'static str) -> anyhow::Result<String> {
    let ast = parse(src)?;
    let mut context = Context::create();
    let module = codegen(ast, &mut context)?;

    let printf_fn = module
        .get_function("printf")
        .ok_or_else(|| anyhow::anyhow!("printf not declared in module"))?;
    let engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
    engine.add_global_mapping(&printf_fn, mock_printf as *const () as usize);
    add_c_lib_mappings(&module, &engine);

    unsafe {
        let r = engine.run_function_as_main(module.get_function("main").unwrap(), &[]);
        assert_eq!(r, 0);
        let output = PRINT_BUFFER.with(|buf| buf.borrow().trim().to_string());
        Ok(output)
    }
}

pub fn should_runtime_error(src: &'static str) -> anyhow::Result<()> {
    PRINT_BUFFER.with(|buf| buf.borrow_mut().clear());
    HAD_RUNTIME_ERROR.set(false);
    let ast = parse(src)?;
    let mut context = Context::create();
    let module = codegen(ast, &mut context)?;

    let engine = module.create_jit_execution_engine(OptimizationLevel::None)?;
    let printf_fn = module
        .get_function("printf")
        .ok_or_else(|| anyhow::anyhow!("printf not declared in module"))?;
    let exit_fn = module.get_function("exit").unwrap();
    engine.add_global_mapping(&printf_fn, mock_printf as *const () as usize);
    engine.add_global_mapping(&exit_fn, mock_exit as *const () as usize);
    add_c_lib_mappings(&module, &engine);

    unsafe {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.run_function_as_main(module.get_function("main").unwrap(), &[]);
        }));
        assert!(HAD_RUNTIME_ERROR.get());
        Ok(())
    }
}
