use clap::Parser;
use rscc::driver::{CompilerOptions, Driver};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "rscc",
    version = "0.1.0",
    about = "A C compiler written in Rust"
)]
struct Cli {
    /// Input C source file
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Output file
    #[arg(short = 'o', value_name = "FILE")]
    output: Option<PathBuf>,

    /// Only run the preprocessor
    #[arg(short = 'E')]
    preprocess_only: bool,

    /// Generate assembly code
    #[arg(short = 'S')]
    assembly_only: bool,

    /// Compile only; do not link
    #[arg(short = 'c')]
    compile_only: bool,

    /// Add directory to include search path
    #[arg(short = 'I', value_name = "DIR", action = clap::ArgAction::Append)]
    include_dirs: Vec<PathBuf>,

    /// Define preprocessor macro
    #[arg(short = 'D', value_name = "MACRO[=VAL]", action = clap::ArgAction::Append)]
    defines: Vec<String>,

    /// Link with library
    #[arg(short = 'l', value_name = "LIB", action = clap::ArgAction::Append)]
    libraries: Vec<String>,

    /// Add directory to library search path
    #[arg(short = 'L', value_name = "DIR", action = clap::ArgAction::Append)]
    library_dirs: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let mut defines = Vec::new();
    for def in cli.defines {
        if let Some((k, v)) = def.split_once('=') {
            defines.push((k.to_string(), v.to_string()));
        } else {
            defines.push((def, "1".to_string()));
        }
    }

    let options = CompilerOptions {
        input_file: cli.input,
        output_file: cli.output,
        include_dirs: cli.include_dirs,
        defines,
        libraries: cli.libraries,
        library_dirs: cli.library_dirs,
        emit_assembly: cli.assembly_only,
        emit_preprocessor: cli.preprocess_only,
        compile_only: cli.compile_only,
    };

    let driver = Driver::new(options);
    if let Err(err) = driver.run() {
        eprintln!("rscc error: {}", err);
        std::process::exit(1);
    }
}
