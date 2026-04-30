use crate::cli::EmittingType;
use clap::Parser;
use inkwell::OptimizationLevel;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use loxc::*;
use std::process::Command;
use inkwell::context::Context;

fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    let file_content = std::fs::read_to_string(&args.filename)?;
    let ast = parser::parse(&file_content)?;
    let mut ctx = Context::create();
    let module = codegen::codegen(ast, &mut ctx)?;

    // EMITTING LLVM: UNIVERSITY ASSIGNMENT ENDS HERE
    if matches!(args.emit, EmittingType::LlvmIr) {
        std::fs::write(args.output_filename(), module.to_string())?;
        return Ok(());
    }

    // rest is just helper for us to test compiler
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
