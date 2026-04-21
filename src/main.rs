use crate::cli::EmittingType;
use clap::Parser;
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use std::process::Command;

mod ast;
mod cli;
mod error;
mod parser;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    let file_content = std::fs::read_to_string(&args.filename)?;
    let _ast = parser::parse(&file_content)?;

    //TODO here do the magic

    let llvm = r#"
        define i32 @main() {
            entry:
              ret i32 0
            }"#;
    // EMITTING LLVM: UNIVERSITY ASSIGNMENT ENDS HERE
    if matches!(args.emit, EmittingType::LlvmIr) {
        std::fs::write(args.output_filename(), llvm)?;
        return Ok(());
    }

    // rest is just helper for us to test compiler
    let llvm_cstring = std::ffi::CString::new(llvm)?;
    let context = Context::create();
    let buffer = MemoryBuffer::create_from_memory_range_copy(llvm_cstring.as_bytes_with_nul(), "input.ll");
    let module = context.create_module_from_ir(buffer)?;
    Target::initialize_all(&InitializationConfig::default());

    let triple = TargetMachine::get_default_triple();
    module.set_triple(&triple);

    let target = Target::from_triple(&triple)?;
    let target_machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| anyhow::anyhow!("Could not create target machine"))?;

    if matches!(args.emit, EmittingType::Obj) {
        target_machine.write_to_file(
            &module,
            inkwell::targets::FileType::Object,
            args.output_filename().as_ref(),
        )?;
        return Ok(());
    }
    if matches!(args.emit, EmittingType::Exe) {
        let obj_path = std::env::temp_dir().join("my_compiler_temp.obj");

        target_machine.write_to_file(&module, inkwell::targets::FileType::Object, &obj_path)?;

        Command::new("clang")
            .args([obj_path.to_str().unwrap(), "-o", &args.output_filename()])
            .status()?;

        std::fs::remove_file(obj_path)?;
        return Ok(());
    }
    unreachable!();
}
