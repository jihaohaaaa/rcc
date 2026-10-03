use crate::codegen::{AArch64Emitter, TargetEmitter, X86_64Emitter};
use crate::diag::Diagnostics;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::preprocessor::Preprocessor;
use crate::sema::Sema;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct CompilerOptions {
    pub input_file: Option<PathBuf>,
    pub output_file: Option<PathBuf>,
    pub include_dirs: Vec<PathBuf>,
    pub defines: Vec<(String, String)>,
    pub libraries: Vec<String>,
    pub library_dirs: Vec<PathBuf>,
    pub emit_assembly: bool,
    pub emit_preprocessor: bool,
    pub compile_only: bool,
    pub target: Option<String>,
}

pub struct Driver {
    pub options: CompilerOptions,
}

impl Driver {
    pub fn new(options: CompilerOptions) -> Self {
        Self { options }
    }

    pub fn compile_source_to_asm(
        &self,
        source: &str,
        filename: &str,
        diag: &mut Diagnostics,
    ) -> Option<String> {
        // Determine target architecture
        let target_arch = match self.options.target.as_deref() {
            Some("x86_64" | "x86-64" | "x64" | "amd64" | "x86" | "i386" | "i686") => "x86_64",
            Some("aarch64" | "arm64" | "arm") => "aarch64",
            Some(other) => {
                diag.error(
                    crate::span::Span::new(0, 0),
                    format!("Unsupported target architecture: '{}'", other),
                );
                return None;
            }
            None => {
                #[cfg(target_arch = "x86_64")]
                {
                    "x86_64"
                }
                #[cfg(not(target_arch = "x86_64"))]
                {
                    "aarch64"
                }
            }
        };

        // 1. Preprocessor
        let mut pp = Preprocessor::new();

        if target_arch == "x86_64" {
            pp.define_object("__x86_64__", "1");
            pp.define_object("__x86_64", "1");
            pp.define_object("__amd64__", "1");
            pp.define_object("__amd64", "1");
        } else {
            pp.define_object("__aarch64__", "1");
            pp.define_object("__arm64__", "1");
            pp.define_object("__arm64", "1");
        }

        // Add user-specified include directories
        for dir in &self.options.include_dirs {
            pp.add_include_path(dir);
        }

        // Add default compiler include directories
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = PathBuf::from(manifest_dir).join("include");
            if p.exists() {
                pp.add_include_path(p);
            }
        }
        let local_inc = Path::new("include");
        if local_inc.exists() {
            pp.add_include_path(local_inc);
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(parent) = exe.parent()
        {
            let inc = parent.join("../include");
            if inc.exists() {
                pp.add_include_path(inc);
            }
        }

        for (k, v) in &self.options.defines {
            pp.define_object(k, v);
        }

        let preprocessed = pp.process(source, filename, diag);
        if diag.has_errors() {
            return None;
        }

        if self.options.emit_preprocessor {
            return Some(preprocessed);
        }

        // 2. Lexer
        eprintln!("[RSCC] Starting Lexer...");
        let mut lexer = Lexer::new(&preprocessed);
        let tokens = lexer.tokenize(diag);
        eprintln!("[RSCC] Finished Lexer: {} tokens", tokens.len());
        if diag.has_errors() {
            eprintln!("{}", diag.render_to_string(filename, &preprocessed));
            return None;
        }

        // 3. Parser
        eprintln!("[RSCC] Starting Parser...");
        let mut parser = Parser::new(&tokens);
        let ast = parser.parse_program(diag);
        eprintln!(
            "[RSCC] Finished Parser: {} functions, {} globals",
            ast.functions.len(),
            ast.globals.len()
        );
        if diag.has_errors() {
            eprintln!("{}", diag.render_to_string(filename, &preprocessed));
            return None;
        }

        // 4. Semantic Analysis
        eprintln!("[RSCC] Starting Sema...");
        let mut sema = Sema::new();
        let typed_ast = sema.analyze_program(ast, diag);
        eprintln!("[RSCC] Finished Sema");
        if diag.has_errors() {
            eprintln!("{}", diag.render_to_string(filename, &preprocessed));
            return None;
        }

        // 5. Codegen
        eprintln!("[RSCC] Starting Codegen for target {}...", target_arch);
        let mut emitter: Box<dyn TargetEmitter> = match target_arch {
            "x86_64" => Box::new(X86_64Emitter::new()),
            _ => Box::new(AArch64Emitter::new()),
        };
        let asm = emitter.emit_program(&typed_ast);
        eprintln!("[RSCC] Finished Codegen: {} bytes asm", asm.len());
        Some(asm)
    }

    pub fn run(&self) -> Result<(), String> {
        let input_path = self
            .options
            .input_file
            .as_ref()
            .ok_or("No input file provided")?;
        let filename = input_path.to_str().unwrap_or("input.c");
        let source = std::fs::read_to_string(input_path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let mut diag = Diagnostics::new();
        let asm_opt = self.compile_source_to_asm(&source, filename, &mut diag);

        if diag.has_errors() {
            diag.print(filename, &source);
            return Err("Compilation failed due to errors".to_string());
        }

        let asm = asm_opt.ok_or("Failed to generate assembly")?;

        if self.options.emit_preprocessor {
            if let Some(out) = &self.options.output_file {
                std::fs::write(out, asm).map_err(|e| e.to_string())?;
            } else {
                print!("{}", asm);
            }
            return Ok(());
        }

        if self.options.emit_assembly {
            if let Some(out) = &self.options.output_file {
                std::fs::write(out, asm).map_err(|e| e.to_string())?;
            } else {
                print!("{}", asm);
            }
            return Ok(());
        }

        // Assembly or Linking via system clang
        let asm_file = tempfile::Builder::new()
            .suffix(".s")
            .tempfile()
            .map_err(|e| e.to_string())?;
        std::fs::write(asm_file.path(), &asm).map_err(|e| e.to_string())?;

        let output_path = self.options.output_file.clone().unwrap_or_else(|| {
            if self.options.compile_only {
                PathBuf::from("a.o")
            } else {
                PathBuf::from("a.out")
            }
        });

        let mut cmd = Command::new("clang");
        cmd.arg("-x").arg("assembler");
        if self.options.compile_only {
            cmd.arg("-c");
        }
        cmd.arg(asm_file.path());
        cmd.arg("-o").arg(&output_path);

        if !self.options.compile_only {
            for dir in &self.options.library_dirs {
                cmd.arg("-L").arg(dir);
            }
            for lib in &self.options.libraries {
                cmd.arg(format!("-l{}", lib));
            }
            cmd.arg("-lm");
        }

        let status = cmd
            .status()
            .map_err(|e| format!("Failed to invoke clang: {}", e))?;
        if !status.success() {
            return Err(format!("clang exited with status: {}", status));
        }

        Ok(())
    }

    pub fn compile_and_run(source: &str) -> Result<i32, String> {
        let mut diag = Diagnostics::new();
        let driver = Driver::new(CompilerOptions::default());
        let asm = driver
            .compile_source_to_asm(source, "test.c", &mut diag)
            .ok_or_else(|| diag.render_to_string("test.c", source))?;

        let asm_file = tempfile::Builder::new()
            .suffix(".s")
            .tempfile()
            .map_err(|e| e.to_string())?;
        std::fs::write(asm_file.path(), &asm).map_err(|e| e.to_string())?;

        let exe_file = tempfile::Builder::new()
            .suffix(".out")
            .tempfile()
            .map_err(|e| e.to_string())?;

        let mut cmd = Command::new("clang");
        cmd.arg("-x").arg("assembler");
        cmd.arg(asm_file.path());
        cmd.arg("-lm");
        cmd.arg("-o").arg(exe_file.path());

        let build_status = cmd.status().map_err(|e| format!("clang failed: {}", e))?;
        if !build_status.success() {
            return Err(format!("clang assembler failed on:\n{}", asm));
        }

        let run_status = Command::new(exe_file.path())
            .status()
            .map_err(|e| format!("Failed to execute test binary: {}", e))?;

        Ok(run_status.code().unwrap_or(-1))
    }
}
