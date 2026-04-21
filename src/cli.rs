use clap::{Parser, ValueEnum};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum EmittingType{
    /// Emits LLVM-IR
    LlvmIr,
    /// Emits object file
    Obj,
    /// Emits executable
    Exe
}
#[derive(Parser)]
pub struct Cli{
    /// Path to lox script
    pub filename: String,
    /// Write output to filename
    #[clap(short, value_name = "FILENAME")]
    output: Option<String>,

    /// Type of output for the compiler to emit
    #[clap(long, value_name = "EMITTING_TYPE")]
    #[arg(default_value = "exe")]
    pub emit: EmittingType,
}

impl Cli {
    pub fn output_filename(&self) -> String {
        if let Some(out) = &self.output {
            return out.clone();
        }

        let path = std::path::Path::new(&self.filename);
        let stem = path.file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        match &self.emit {
            EmittingType::LlvmIr => format!("{stem}.ll"),
            EmittingType::Obj => format!("{stem}.o"),
            EmittingType::Exe =>{
                if cfg!(windows) {
                    format!("{stem}.exe")
                }
                else {
                    format!("{stem}")
                }
            }
        }
    }
}