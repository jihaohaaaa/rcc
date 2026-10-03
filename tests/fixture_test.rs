use rscc::driver::{CompilerOptions, Driver};
use std::path::PathBuf;
use std::process::Command;

fn run_fixture_file(rel_path: &str, expected_exit_code: i32) {
    run_fixture_file_with_args(rel_path, &[], expected_exit_code);
}

fn run_fixture_file_with_args(rel_path: &str, args: &[&str], expected_exit_code: i32) {
    let rel_path_owned = rel_path.to_string();
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let rel_path = &rel_path_owned;
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            let root_dir = PathBuf::from(manifest_dir);
            let fixture_dir = root_dir.join("tests/fixtures");
            let include_dir = root_dir.join("include");
            let c_file = fixture_dir.join(rel_path);

            let source = std::fs::read_to_string(&c_file)
                .unwrap_or_else(|e| panic!("Failed to read fixture file {:?}: {}", c_file, e));

            let options = CompilerOptions {
                input_file: Some(c_file.clone()),
                include_dirs: vec![fixture_dir, include_dir, root_dir],
                ..Default::default()
            };

            let driver = Driver::new(options);
            let mut diag = rscc::diag::Diagnostics::new();
            let asm = driver
                .compile_source_to_asm(&source, c_file.to_str().unwrap(), &mut diag)
                .unwrap_or_else(|| panic!("Compilation failed for {:?}:\n{}", c_file, diag.render_to_string(c_file.to_str().unwrap(), &source)));

    let asm_file = tempfile::Builder::new().suffix(".s").tempfile().unwrap();
    let asm_path = asm_file.into_temp_path();
    std::fs::write(&asm_path, &asm).unwrap();

    let exe_file = tempfile::Builder::new().suffix(".out").tempfile().unwrap();
    let exe_path = exe_file.into_temp_path();

    let mut cmd = Command::new("clang");
    cmd.arg("-x").arg("assembler");
    cmd.arg(&asm_path);
    cmd.arg("-lm");
    cmd.arg("-o").arg(&exe_path);

    let build_output = cmd.output().expect("Failed to execute clang assembler");
    if !build_output.status.success() {
        let err = String::from_utf8_lossy(&build_output.stderr);
        panic!("Clang assembly failed:\n{}", err);
    }

    let mut run_cmd = Command::new(&exe_path);
    for a in &args_owned {
        run_cmd.arg(a);
    }
    let output = run_cmd.output().expect("Failed to execute binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{}", stdout);

    if output.status.code() != Some(expected_exit_code) {
        let lldb_out = Command::new("lldb")
            .arg("-b")
            .arg("-o").arg("target stop-hook add -o 'register read lr' -o 'bt'")
            .arg("-o").arg("process launch")
            .arg(&exe_path)
            .output();
        let bt = if let Ok(o) = lldb_out {
            String::from_utf8_lossy(&o.stdout).to_string()
        } else {
            String::new()
        };
        panic!("Failed on fixture file: {}\nStatus: {:?}\nStdout: {}\nStderr: {}\nLLDB Backtrace:\n{}", rel_path, output.status, stdout, stderr, bt);
    }
    })
    .unwrap();
    handle.join().unwrap();
}

#[test]
fn test_fixture_expressions() {
    run_fixture_file("expressions.c", 0);
}

#[test]
fn test_fixture_control_flow() {
    run_fixture_file("control_flow.c", 0);
}

#[test]
fn test_fixture_functions() {
    run_fixture_file("functions.c", 0);
}

#[test]
fn test_fixture_pointers() {
    run_fixture_file("pointers.c", 0);
}

#[test]
fn test_fixture_types() {
    run_fixture_file("types.c", 0);
}

#[test]
fn test_fixture_preprocessor() {
    run_fixture_file("preprocessor.c", 0);
}

#[test]
fn test_fixture_stdlib() {
    run_fixture_file("stdlib.c", 0);
}

#[test]
fn test_fixture_algorithms() {
    run_fixture_file("algorithms.c", 0);
}

#[test]
fn test_fixture_matrix() {
    run_fixture_file("matrix.c", 0);
}

#[test]
fn test_fixture_demo() {
    run_fixture_file("demo.c", 0);
}

#[test]
fn test_fixture_quickjs_cutils() {
    run_fixture_file("quickjs_cutils_test.c", 0);
}

#[test]
fn test_fixture_quickjs_regexp() {
    run_fixture_file("quickjs_regexp_test.c", 0);
}

#[test]
fn test_fixture_quickjs_eval() {
    run_fixture_file("quickjs_eval_test.c", 0);
}

#[test]
fn test_fixture_quickjs_cli() {
    run_fixture_file_with_args(
        "quickjs_cli_test.c",
        &["-e", "console.log('RSCC_QJS_CLI_SUCCESS');"],
        0,
    );
}

#[test]
fn test_fixture_sqlite3() {
    run_fixture_file("sqlite3_test.c", 0);
}
