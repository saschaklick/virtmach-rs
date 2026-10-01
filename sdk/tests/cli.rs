//! Runs the sdk binaries the way they are used from the command line, from the repository root
//! so ./include/ holds the interrupt csv files.

use std::{ fs, path::{ Path, PathBuf }, process::{ Command, Output } };

const COMPILER: &str = env!("CARGO_BIN_EXE_compiler");
const BASIC: &str = env!("CARGO_BIN_EXE_basic");
const RUNTIME: &str = env!("CARGO_BIN_EXE_runtime");

/// Interrupts the runtime provides without a surface, the order does not matter: built-in
/// interrupts are numbered by their INDEX
const PLAIN: [&str; 4] = ["proc", "math", "random", "trig"];
/// Interrupts the runtime provides with the term surface
const TERM: [&str; 6] = ["proc", "math", "string", "random", "surface", "trig"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn program(file: &str) -> String {
    root().join("examples/programs").join(file).display().to_string()
}

/// A fresh directory for one test's files
fn tmp_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli").join(test);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(bin: &str, args: &[&str], cwd: &Path) -> Output {
    Command::new(bin).args(args).current_dir(cwd).output().unwrap_or_else(|e| panic!("could not start {}: {}", bin, e))
}

fn stdout(out: &Output) -> String { String::from_utf8_lossy(&out.stdout).to_string() }

fn stderr(out: &Output) -> String { String::from_utf8_lossy(&out.stderr).to_string() }

fn assert_ok(out: &Output) {
    assert!(out.status.success(), "exit {:?}\nstdout:\n{}\nstderr:\n{}", out.status, stdout(out), stderr(out));
}

fn assert_fails(out: &Output, expected: &str) {
    assert!(!out.status.success(), "succeeded\nstdout:\n{}", stdout(out));
    assert!(stderr(out).contains(expected), "expected {:?} in stderr:\n{}", expected, stderr(out));
}

/// Compiles a source file with a bin into dir/name.bin and returns its path
fn compile(bin: &str, source: &str, interrupts: &[&str], dir: &Path, name: &str) -> String {
    let out_file = dir.join(format!("{}.bin", name)).display().to_string();
    let mut args = vec![source];
    args.extend(interrupts);
    args.extend(["-o", &out_file, "-v", "0"]);
    let out = run(bin, &args, &root());
    assert_ok(&out);
    assert!(stdout(&out).contains(&format!("[OK] wrote binary to {}", out_file)), "{}", stdout(&out));
    out_file
}

#[test]
fn compiler_assembles_listing() {
    let dir = tmp_dir("compiler_assembles_listing");
    let bin = compile(COMPILER, &program("count.txt"), &["proc", "math"], &dir, "count");
    let data = fs::read(&bin).unwrap();
    assert!(data.len() > 2, "{} bytes", data.len());
}

#[test]
fn compiler_disassembles_when_verbose() {
    let dir = tmp_dir("compiler_disassembles_when_verbose");
    let out_file = dir.join("count.bin").display().to_string();
    let out = run(COMPILER, &[&program("count.txt"), "proc", "math", "-o", &out_file], &root());
    assert_ok(&out);
    let text = stdout(&out);
    assert!(text.contains("Program \"count\""), "{}", text);
    assert!(text.contains("hlt") && text.contains("jmp"), "{}", text);
}

#[test]
fn compiler_default_output_and_repo_include() {
    // run somewhere without ./include/, the csv files come from the repository's include/ and the
    // binary is named after the source
    let dir = tmp_dir("compiler_default_output_and_repo_include");
    let out = run(COMPILER, &[&program("count.txt"), "proc", "math", "-v", "0"], &dir);
    assert_ok(&out);
    assert!(dir.join("count.bin").is_file());

    // cargo run inside sdk/
    let pong = dir.join("pong.bin").display().to_string();
    let out = run(COMPILER, &["../examples/programs/pong.bas", "proc", "math", "string", "random", "surface", "-o", &pong, "-v", "0"], &root().join("sdk"));
    assert_ok(&out);
    assert!(Path::new(&pong).is_file());
}

#[test]
fn compiler_include_dir() {
    // an interrupt that only exists in the -I directory
    let dir = tmp_dir("compiler_include_dir");
    let include = dir.join("extra");
    fs::create_dir_all(&include).unwrap();
    fs::copy(root().join("include/math.csv"), include.join("maths.csv")).unwrap();
    let source = dir.join("maths.txt");
    fs::write(&source, "r0 = maths.div(#6, #2)\nend\n").unwrap();
    let out_file = dir.join("maths.bin").display().to_string();
    let args = [source.display().to_string(), "proc".into(), "maths".into(), "-o".into(), out_file.clone(), "-I".into(), include.display().to_string(), "-v".into(), "0".into()];
    let out = run(COMPILER, &args.iter().map(String::as_str).collect::<Vec<_>>(), &dir);
    assert_ok(&out);
    assert!(Path::new(&out_file).is_file());
}

#[test]
fn compiler_accepts_basic() {
    let dir = tmp_dir("compiler_accepts_basic");
    let from_compiler = compile(COMPILER, &program("sum.bas"), &PLAIN, &dir, "compiler");
    let from_basic = compile(BASIC, &program("sum.bas"), &PLAIN, &dir, "basic");
    assert_eq!(fs::read(from_compiler).unwrap(), fs::read(from_basic).unwrap());
}

#[test]
fn compiler_errors() {
    let dir = tmp_dir("compiler_errors");
    let out_file = dir.join("out.bin").display().to_string();

    let out = run(COMPILER, &[&program("count.txt"), "proc", "nosuch", "-o", &out_file], &root());
    assert_fails(&out, "could not find nosuch.csv");

    let bad = dir.join("bad.txt");
    fs::write(&bad, "reg r0\nfoo #1\n").unwrap();
    let out = run(COMPILER, &[&bad.display().to_string(), "proc", "-o", &out_file], &root());
    assert_fails(&out, "illegal op code: foo");
    assert!(stderr(&out).contains("line #2"), "{}", stderr(&out));

    let out = run(COMPILER, &[&dir.join("missing.txt").display().to_string(), "proc", "-o", &out_file], &root());
    assert_fails(&out, "could not open file");
    assert!(!Path::new(&out_file).exists());
}

#[test]
fn basic_writes_listing() {
    let dir = tmp_dir("basic_writes_listing");
    let listing = dir.join("sum.txt").display().to_string();
    let out_file = dir.join("sum.bin").display().to_string();
    let out = run(BASIC, &[&program("sum.bas"), "proc", "math", "random", "-o", &out_file, "-l", &listing], &root());
    assert_ok(&out);
    let text = stdout(&out);
    assert!(text.contains(&format!("[OK] wrote listing to {}", listing)), "{}", text);
    assert!(text.contains("SUM") && text.contains("FACT"), "variables missing:\n{}", text);
    let listing = fs::read_to_string(&listing).unwrap();
    assert!(listing.contains("end"), "{}", listing);
    assert!(Path::new(&out_file).is_file());

    // the written listing assembles to the same binary
    let reassembled = compile(COMPILER, &dir.join("sum.txt").display().to_string(), &PLAIN, &dir, "reassembled");
    assert_eq!(fs::read(reassembled).unwrap(), fs::read(out_file).unwrap());
}

#[test]
fn basic_errors() {
    let dir = tmp_dir("basic_errors");
    let out_file = dir.join("out.bin").display().to_string();

    let bad = dir.join("bad.bas");
    fs::write(&bad, "REM\nFOR I = 1\n").unwrap();
    let out = run(BASIC, &[&bad.display().to_string(), "proc", "-o", &out_file], &root());
    assert_fails(&out, "line 2: syntax error");

    let out = run(BASIC, &[&program("strings.bas"), "proc", "-o", &out_file], &root());
    assert_fails(&out, "surface");

    let out = run(BASIC, &[&dir.join("missing.bas").display().to_string(), "proc", "-o", &out_file], &root());
    assert_fails(&out, "could not read");
    assert!(!Path::new(&out_file).exists());
}

#[test]
fn runtime_runs_to_end() {
    let dir = tmp_dir("runtime_runs_to_end");
    let bin = compile(BASIC, &program("sum.bas"), &PLAIN, &dir, "sum");
    let out = run(RUNTIME, &[&bin, "-v", "0"], &root());
    assert_ok(&out);
    assert!(stdout(&out).contains(&format!("loaded binary {}", bin)), "{}", stdout(&out));
}

#[test]
fn runtime_term_surface() {
    let dir = tmp_dir("runtime_term_surface");
    let bin = compile(BASIC, &program("strings.bas"), &TERM, &dir, "strings");
    let out = run(RUNTIME, &[&bin, "-s", "term", "-v", "0"], &root());
    assert_ok(&out);
}

#[test]
fn interrupt_order_does_not_matter() {
    // built-in interrupts are numbered by their INDEX, not by their position
    let dir = tmp_dir("interrupt_order_does_not_matter");
    let forward = compile(BASIC, &program("dice.bas"), &["proc", "math", "random"], &dir, "forward");
    let backward = compile(BASIC, &program("dice.bas"), &["random", "math", "proc"], &dir, "backward");
    assert_eq!(fs::read(forward).unwrap(), fs::read(backward).unwrap());
}

#[test]
fn runtime_errors() {
    let dir = tmp_dir("runtime_errors");

    // without -s term the surface slot holds dummy
    let source = dir.join("surface.bas");
    fs::write(&source, "REM\nsurface.clear(0)\nEND\n").unwrap();
    let bin = compile(BASIC, &source.display().to_string(), &["proc", "math", "random", "trig", "surface"], &dir, "surface");
    let out = run(RUNTIME, &[&bin, "-v", "0"], &root());
    assert_fails(&out, "runtime error: UnimplementedInterruptFunc");

    let out = run(RUNTIME, &[&dir.join("missing.bin").display().to_string()], &root());
    assert_fails(&out, "failed to open binary");
}
