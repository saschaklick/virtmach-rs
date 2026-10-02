//! Command line front end of the BASIC compiler in virtmach::basic.

use std::{ ffi::OsStr, fs::File, io::{ Read, Write }, path::Path };
use clap::Parser as ClapParser;
use simple_logger;
use virtmach::{ VirtMach, VMAtom, Program, ListingError, basic::{ compile, Loc } };
use interrupt_csv::load_interrupts;

#[derive(clap::Parser, Debug)]
#[command(name = "virtmach-rs BASIC Compiler")]
#[command(version = "0.1")]
#[command(about = "Compile BASIC programs into virtmach-rs binaries", long_about = None)]
struct Args {
    #[arg(help = "BASIC file to compile")]
    source: String,

    #[arg(value_delimiter = ' ', num_args = 1.., help = "Interrupts to include, in the order of the runtime. Reads from a .csv file by the same name")]
    interrupts: Option<Vec<String>>,

    #[arg(short = 'I', long = "include", help = "Additional directory to search for interrupt .csv files, after the source file's directory and ./include/, before the repository's include/ (repeatable)")]
    include: Vec<String>,

    #[arg(short, long, help = "Optional binary output file")]
    output: Option<String>,

    #[arg(short, long, help = "Optional file to write the generated virtmach listing to")]
    listing: Option<String>,

    #[arg(short, long, default_value_t = 1, help = "Verbosity level")]
    verbose: u32
}


// ---------------------------------------------------------------------------------------------
// Command line

/// Lists the instructions, addresses match the vm's program counter (the leading atom-size byte is skipped).
fn disassemble(program: &Program) {
    println!();
    println!("\tatom size id {:02X}", program.data[0]);
    println!();
    let code = Program { source: program.source, id: program.id, data: program.data };
    let hex_wid = (1 + size_of::<VMAtom>()) * 3 - 1;
    let mut pos = 0usize;
    let instructions = code.get_instructions();
    while pos < instructions.len() {
        let mut op = String::new();
        let addr = pos;
        pos = VirtMach::decompile(&code, pos, &mut op);
        let bytes = instructions[addr..pos].iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
        let target = match (op.split_whitespace().next(), op.split('#').nth(1).map(|v| v.parse::<i64>())) {
            (Some("cal" | "jmp" | "jpz" | "jpc" | "jps"), Some(Ok(offset))) => format!("  -> {:04x}", pos as i64 + offset),
            _ => String::new()
        };
        println!("\t{:04x} | {:hex_wid$} | {}{}", addr, bytes, op, target);
    }
    println!();
}

fn listing_error_line(err: &ListingError) -> Option<usize> {
    match err {
        ListingError::IllegalOp(l, _) | ListingError::IllegalArgument(l, _) | ListingError::IllegalRegister(l, _)
        | ListingError::IllegalInterrupt(l, _) | ListingError::MalformedDefine(l, _) | ListingError::IllegalDefineValue(l, _)
        | ListingError::UnknownLabel(l, _) | ListingError::UnknownInterrupt(l, _) | ListingError::UnknownFunction(l, _)
        | ListingError::MalformedFunction(l, _) | ListingError::MalformedLiteral(l, _) => Some(*l),
        ListingError::Basic(l, _) => Some(*l).filter(|l| *l > 0),
        ListingError::NoError => None
    }
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    match args.verbose {
        0 => {}
        _ => simple_logger::init_with_level(match args.verbose {
            1 => log::Level::Error,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            _ => log::Level::Trace
        }).unwrap()
    }

    let source = Path::new(&args.source);
    let name = source.file_stem().unwrap_or(OsStr::new("n/a")).to_string_lossy().to_string();
    let out_file = args.output.unwrap_or(format!("{}.bin", source.file_stem().unwrap_or(OsStr::new("out")).to_string_lossy()));

    let (interrupts, functions) = load_interrupts(&interrupt_csv::search_path(source, &args.include), &args.interrupts.unwrap_or(vec![]))
        .map_err(|e| { eprintln!(); eprintln!("[ERROR] {}", e); eprintln!(); String::new() })?;

    let mut content = String::new();
    File::open(source).and_then(|mut f| f.read_to_string(&mut content))
        .map_err(|e| { eprintln!(); eprintln!("[ERROR] could not read {}: {}", args.source, e); eprintln!(); String::new() })?;

    let output = match compile(&name, &content, &interrupts, &functions) {
        Ok(output) => output,
        Err(e) => {
            eprintln!();
            eprintln!("[ERROR] line {}: {}", e.line, e.msg);
            if e.line > 0 { eprintln!(); eprintln!("\t{}: {}", e.line, content.lines().nth(e.line - 1).unwrap_or("").trim()); }
            eprintln!();
            return Err(String::new());
        }
    };

    if let Some(listing_file) = &args.listing {
        File::create(listing_file).and_then(|mut f| f.write_all(output.listing.as_bytes()))
            .map_err(|e| { eprintln!("[ERROR] failed to write listing {}: {}", listing_file, e); String::new() })?;
        println!("[OK] wrote listing to {}", listing_file);
    }

    if args.verbose >= 2 {
        println!();
        println!("{}", output.listing);
    }

    match VirtMach::compile_owned(&name, &output.listing, &interrupts, functions.clone()) {
        Ok((program, _)) => {
            if args.verbose > 0 {
                println!();
                println!("Program \"{}\" ({}b, {} labels, {} jumps, {} memory cells):", name, program.data.len(), output.labels, output.jumps, output.memory_cells());
                for (var, loc) in &output.variables {
                    if var.starts_with('_') { continue; }
                    match loc { Loc::Reg(r) => println!("\t{:10} r{}", var, r), Loc::Mem(a) => println!("\t{:10} memory #{}", var, a) }
                }
                for (array, base, size) in &output.arrays {
                    println!("\t{:10} memory #{}..#{}", format!("{}()", array), base, base + size - 1);
                }
                disassemble(&program);
            }
            File::create(&out_file).and_then(|mut f| f.write_all(program.data))
                .map_err(|e| { eprintln!("failed to write binary file {}: {}", out_file, e); String::new() })?;
            println!("[OK] wrote binary to {}", &out_file);
            Ok(())
        }
        Err(e) => {
            eprintln!();
            eprintln!("[ERROR] generated listing failed to assemble: {:?}", e);
            if let Some(l) = listing_error_line(&e) {
                eprintln!(); eprintln!("\tlisting line #{}: {:?}", l, output.listing.lines().nth(l - 1).unwrap_or(""));
            }
            eprintln!();
            Err(String::new())
        }
    }
}

// ---------------------------------------------------------------------------------------------

#[path = "../lib/interrupt_csv.rs"]
mod interrupt_csv;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use virtmach::basic::{ Error, Output };
    use crate::interrupt_csv::Functions;
    use virtmach::{ Runtime, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction, proc, math, random, surface, string, trig, dummy } };

    const INTERRUPTS: [&str; 6] = ["proc", "math", "random", "surface", "string", "trig"];

    /// Records surface calls instead of drawing.
    struct MockSurface { calls: usize, texts: Vec<String> }

    impl SoftInterrupt for MockSurface {
        fn name(&self) -> &str { surface::NAME }

        fn functions(&self) -> &'static [SoftInterruptFunction<'static>] { &surface::FUNCTIONS }

        fn call(&mut self, vm: &mut VirtMach) {
            let op = vm.stack_pop();
            let function = surface::FUNCTIONS.iter().find(|f| f.no == op).expect("unknown surface function");
            let args: Vec<VMAtom> = (0..function.arguments).map(|_| vm.stack_pop()).collect();
            match op {
                10 => self.texts.push(vm.get_str(args[3] as u8).to_string()),
                15 => { let width = vm.get_bin(args[1] as u8).len() as VMAtom * 4; vm.stack_push(width); vm.stack_push(5); }
                16 => { vm.stack_push(64); vm.stack_push(40); }
                17 => { vm.stack_push(5); vm.stack_push(5); }
                18 => { for v in [0, 0, 64, 40] { vm.stack_push(v); } }
                _ => {}
            }
            self.calls += 1;
        }
    }

    struct Run {
        output: Output,
        registers: Vec<VMAtom>,
        memory: Vec<VMAtom>,
        state: Runtime,
        error: RuntimeError,
        surface_calls: usize,
        texts: Vec<String>,
        /// string entries 0..64 after the run
        strings: Vec<String>
    }

    impl Run {
        fn get(&self, name: &str) -> VMAtom {
            match self.output.variables.iter().find(|v| v.0 == name).unwrap_or_else(|| panic!("no variable {}", name)).1 {
                Loc::Reg(r) => self.registers[r as usize],
                Loc::Mem(a) => self.memory[a]
            }
        }

        fn array(&self, name: &str) -> Vec<VMAtom> {
            let (_, base, size) = self.output.arrays.iter().find(|a| a.0 == name).unwrap();
            self.memory[*base..base + size].to_vec()
        }
    }

    fn include_dirs() -> Vec<PathBuf> {
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../include")]
    }

    fn programs_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/programs")
    }

    fn build(src: &str) -> Result<Output, Error> {
        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        compile("test", src, &interrupts, &functions)
    }

    fn run_src(src: &str, halts: usize) -> Run {
        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let output = compile("test", src, &interrupts, &functions).unwrap_or_else(|e| panic!("line {}: {}", e.line, e.msg));
        let (program, _) = VirtMach::compile_owned("test", &output.listing, &interrupts, functions.clone())
            .unwrap_or_else(|e| panic!("{:?}\n{}", e, output.listing));

        let mut vm = VirtMach::new();
        vm.load_program(program);
        let (mut p, mut m, mut r, mut s, mut t, mut g) = (proc::Interrupt {}, math::Interrupt {}, random::Interrupt {}, MockSurface { calls: 0, texts: vec![] }, string::Interrupt {}, trig::Interrupt {});
        let [mut time, mut gpio, mut uart, mut i2c, mut pwm] = [dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}, dummy::Interrupt {}];
        {
            // at their INDEX like the runtime, the numbers come from load_interrupts
            let mut ints: [&mut dyn SoftInterrupt; 11] = [&mut m, &mut p, &mut t, &mut r, &mut time, &mut g, &mut s, &mut gpio, &mut uart, &mut i2c, &mut pwm];
            for (slot, int) in ints.iter().enumerate() {
                assert!(virtmach::interrupts::builtin_index(int.name()).is_none_or(|index| index as usize == slot), "{} in slot {}", int.name(), slot);
            }
            for _ in 0..=halts {
                vm.run(1_000_000, &mut ints);
                if vm.state == Runtime::Stp || vm.state == Runtime::Err { break; }
            }
        }
        let error = vm.error.clone();
        let strings = (0..64u8).map(|i| if vm.has_bin(i) { vm.get_str(i).to_string() } else { String::new() }).collect();
        Run { registers: vm.registers.to_vec(), memory: vm.memory.to_vec(), state: vm.state, error, surface_calls: s.calls, texts: s.texts, strings, output }
    }

    fn run_file(file: &str, halts: usize) -> Run {
        let src = std::fs::read_to_string(programs_dir().join(file)).unwrap();
        let run = run_src(&src, halts);
        assert_eq!(run.error, RuntimeError::NoError, "{}:\n{}", file, run.output.listing);
        run
    }

    fn assert_finished(run: &Run) {
        assert_eq!(run.state, Runtime::Stp, "program did not reach END\n{}", run.output.listing);
    }

    #[test]
    fn sum() {
        let run = run_file("ci/sum.bas", 0);
        assert_finished(&run);
        assert_eq!(run.get("SUM"), 55);
        assert_eq!(run.get("FACT"), 5040);
        assert_eq!(run.get("EVENS"), 30);
    }

    #[test]
    fn primes() {
        let run = run_file("ci/primes.bas", 0);
        assert_finished(&run);
        assert_eq!(run.get("COUNT"), 15);
        assert_eq!(run.get("LAST"), 47);
    }

    #[test]
    fn gcd() {
        let run = run_file("ci/gcd.bas", 0);
        assert_finished(&run);
        assert_eq!((run.get("G1"), run.get("G2"), run.get("G3")), (6, 1, 12));
    }

    #[test]
    fn bits() {
        let run = run_file("ci/bits.bas", 0);
        assert_finished(&run);
        let expected = [("DIRECT", 0), ("BAND", 48), ("BOR", 252), ("BXOR", 204), ("BNOT", -1), ("MASK", -1),
            ("SHL", 16), ("SHR", 64), ("QUOT", 14), ("REMD", 2), ("PREC", 12), ("PAREN", -20), ("TRUTH", -2), ("BITS", 5)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
        assert!(matches!(run.output.variables.iter().find(|v| v.0 == "BITS").unwrap().1, Loc::Mem(_)));
    }

    #[test]
    fn fib() {
        let run = run_file("ci/fib.bas", 0);
        assert_finished(&run);
        assert_eq!(run.array("F"), vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89]);
        assert_eq!(run.get("TOTAL"), 232);
    }

    #[test]
    fn dice() {
        let run = run_file("ci/dice.bas", 0);
        assert_finished(&run);
        let (lo, hi, doubles) = (run.get("LO"), run.get("HI"), run.get("DOUBLES"));
        assert!(2 <= lo && lo <= hi && hi <= 12, "lo {} hi {}", lo, hi);
        assert!((0..=100).contains(&doubles));
    }

    #[test]
    fn bounce() {
        let run = run_file("gfx/bounce.bas", 200);
        assert_eq!(run.state, Runtime::Hlt);
        let (x, y) = (run.get("X"), run.get("Y"));
        assert!((1..=62).contains(&x) && (1..=38).contains(&y), "x {} y {}", x, y);
        assert!(run.surface_calls > 600);
    }

    #[test]
    fn ellipse() {
        // the first frame ends at 360 degrees of the unturned ellipse: get_size, set_clip, clear and 36 lines
        let run = run_file("gfx/ellipse.bas", 0);
        assert_eq!(run.state, Runtime::Hlt);
        assert_eq!((run.get("X"), run.get("Y")), (32 + 19, 20));
        assert_eq!(run.surface_calls, 39);
        // turned by 30 * 3 degrees the same point is at the bottom
        let run = run_file("gfx/ellipse.bas", 30);
        assert_eq!((run.get("X"), run.get("Y")), (32, 20 + 19));
    }

    #[test]
    fn pong() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("gfx/pong.bas", 3000);
        assert_eq!(run.state, Runtime::Hlt);
        let (ly, ry, bx, by) = (run.get("LY"), run.get("RY"), run.get("BX"), run.get("BY"));
        assert!((0..=35).contains(&ly) && (0..=35).contains(&ry), "paddles {} {}", ly, ry);
        assert!((-1..=64).contains(&bx) && (0..=39).contains(&by), "ball {} {}", bx, by);
        assert!((0..=9).contains(&run.get("SL")) && (0..=9).contains(&run.get("SR")));
        assert!(run.output.strings.is_empty(), "{:?}", run.output.strings);
        let last: Vec<String> = run.texts[run.texts.len() - 2..].to_vec();
        assert_eq!(last, [run.get("SL").to_string(), run.get("SR").to_string()]);
    }

    #[test]
    fn strings() {
        let run = run_file("ci/strings.bas", 0);
        assert_finished(&run);
        let expected = [("HELLO", 0), ("SAME", 0), ("OTHER", 1), ("LEN1", 13), ("LEN2", 12), ("LEN3", 0)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
        assert_eq!(run.output.strings, ["Hello, World!", "Hi; \"quoted\"", "", "same entry"]);
        assert_eq!(run.surface_calls, 2);
        assert!(run.output.listing.contains("#db \"Hi\", 0x3b, 0x20, 0x22"), "{}", run.output.listing);

        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let (program, _) = VirtMach::compile_owned("strings", &run.output.listing, &interrupts, functions).unwrap();
        assert_eq!(program.get_fixed_bin_count(), 4);
        for (i, text) in run.output.strings.iter().enumerate() { assert_eq!(program.get_bin(i as u8), text.as_bytes()); }

        let many: String = (0..256).map(|i| format!("S = \"{}\"\n", i)).collect();
        assert!(build(&many).err().unwrap().msg.contains("too many different strings"));
    }

    /// DICT with the dynamic strings is one global, tests writing to it must not run at the same time
    static DICT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Counts the bytes allocated and not yet freed by the current thread
    struct CountingAllocator;

    thread_local! { static LIVE_BYTES: std::cell::Cell<isize> = const { std::cell::Cell::new(0) }; }

    unsafe impl std::alloc::GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let _ = LIVE_BYTES.try_with(|b| b.set(b.get() + layout.size() as isize));
            unsafe { std::alloc::System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
            let _ = LIVE_BYTES.try_with(|b| b.set(b.get() - layout.size() as isize));
            unsafe { std::alloc::System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn dynamic_strings_are_freed() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let live_after = |n: usize| {
            let before = LIVE_BYTES.with(|b| b.get());
            let run = run_src(&format!("REM\nFOR I = 1 TO {}\n  S$ = STR$(I) + \"x\"\nNEXT I\nEND", n), 0);
            assert_eq!(run.error, RuntimeError::NoError);
            drop(run);
            LIVE_BYTES.with(|b| b.get()) - before
        };
        live_after(100);
        let (small, large) = (live_after(100), live_after(5000));
        assert!(large - small < 256, "{} bytes more still allocated after 5000 instead of 100 string operations", large - small);

        let run = run_src("REM\nA$ = STR$(12345)\nstring.substr(A$, A$, 1, 3)\nEND", 0);
        assert_eq!(run.strings[run.get("A$") as usize], "23");
    }

    #[test]
    fn dictionary_out_of_bound() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let cases = [
            ("reading an entry that was never set", "REM\nL = string.get_length(200)\nEND"),
            ("writing a fixed #db entry", "REM\nA = \"fixed\"\nstring.format(A, 5)\nEND"),
            ("joining an entry that was never set", "REM\nA$ = STR$(1) + \"x\"\nstring.concat(A$, A$, 201)\nEND"),
        ];
        for (what, src) in cases {
            let run = run_src(src, 0);
            assert_eq!(run.error, RuntimeError::DictionaryOutOfBound, "{}", what);
        }
        let run = run_src("REM\nA$ = STR$(1)\nL = LEN(A$) + LEN(\"ab\")\nEND", 0);
        assert_eq!((run.error.clone(), run.get("L")), (RuntimeError::NoError, 3));
    }

    #[test]
    fn string_functions() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/strfuncs.bas", 0);
        assert_finished(&run);
        let text = |name: &str| run.strings[run.get(name) as usize].clone();
        let expected = [("A$", "1234"), ("B$", "-56"), ("M$", "World"), ("R$", "World!"), ("F$", "He"), ("T$", "bc"), ("X$", "abc"), ("D$", "2"),
            ("MIN$", "-32768"), ("ZERO$", "0"), ("C$", "Hello, World!"),
            ("J$", "Hello, World!"), ("S$", "123")];
        for (name, value) in expected { assert_eq!(text(name), value, "{}", name); }
        assert_eq!(run.get("L"), 4);
        assert_eq!(run.texts, ["Hello, World!"]);
        assert!(run.get("A$") as usize >= run.output.strings.len(), "results are dynamic entries after the #db entries");

        let e = compile("test", "A$ = STR$(1)", &[], &Functions::new()).err().unwrap();
        assert!(e.msg.contains("add string to the interrupts"), "{}", e.msg);
        let many: String = (0..256).map(|i| format!("A$ = STR$({})\n", i)).collect();
        assert!(build(&many).err().unwrap().msg.contains("at most 255 fit the string indices"));
        assert!(build("A$ = MID$(\"x\")").err().unwrap().msg.contains("MID$ takes"));
        assert!(build("A$ = \"a\" + 1").err().unwrap().msg.contains("cannot join a string and a number"));
        assert!(build("N = 1 : A$ = N + \"a\"").err().unwrap().msg.contains("cannot join a string and a number"));
        assert!(!build("A$ = \"a\" : A$ = A$ + \"b\"").unwrap().listing.contains("add #"), "string join must not become a native add");
    }

    #[test]
    fn control_flow() {
        let run = run_file("ci/control.bas", 0);
        assert_finished(&run);
        let expected = [("SIZES", 1234), ("KINDS", 11234), ("CS", 2), ("DOWHILE", 10), ("DOUNTIL", 12), ("DU", 5),
            ("LOOPWHILE", 101), ("FOUND", 7), ("EXITDO", 4), ("EXITWHILE", 6), ("ONSUM", 111), ("STEPS", 18), ("BACK", 5)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
    }

    #[test]
    fn subroutines() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_file("ci/subs.bas", 0);
        assert_finished(&run);
        let expected = [("G", 12), ("F", 120), ("TOTAL", 24), ("C1", 2), ("SQ", 13), ("A", 2), ("B", 1), ("S", -99), ("M", -11), ("R", 5)];
        for (name, value) in expected { assert_eq!(run.get(name), value, "{}", name); }
        assert_eq!(run.strings[run.get("H$") as usize], "Hello, World!");
        // GCD's parameters are its own, not the main program's A and B
        assert!(run.output.variables.iter().any(|v| v.0 == "GCD.A"));
    }

    #[test]
    fn robust_comparisons() {
        let _lock = DICT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let run = run_src("REM
            A = 30000 : B = -30000 : R = 0
            IF A > B THEN R = R + 1
            IF B < A THEN R = R + 2
            IF NOT (A < B) THEN R = R + 4
            IF A >= B AND B <= A THEN R = R + 8
            LOW = -32767 - 1 : HIGH = 32767
            IF LOW < HIGH AND HIGH > LOW THEN R = R + 16
            V = (A > B) + (B > A)
            LO = MIN(A, B) : HI = MAX(A, B)
            S = 0
            IF \"abc\" < \"abd\" THEN S = S + 1
            IF \"b\" > \"abc\" THEN S = S + 2
            IF STR$(12) = \"12\" THEN S = S + 4
            IF \"ab\" < \"abc\" THEN S = S + 8
            IF \"x\" <> \"x\" THEN S = S + 100
            X$ = \"hi\" : Y$ = STR$(5)
            IF X$ = \"hi\" AND Y$ = \"5\" AND X$ <> Y$ THEN S = S + 16
            END", 0);
        assert_finished(&run);
        assert_eq!((run.get("R"), run.get("V"), run.get("S")), (31, -1, 31));
        assert_eq!((run.get("LO"), run.get("HI")), (-30000, 30000));

        let mut src = String::from("REM\nN = 0\n");
        for i in 0..300 { src.push_str(&format!("IF N = {} THEN N = N + 1\n", i)); }
        src.push_str("END\n");
        let run = run_src(&src, 0);
        assert!(run.output.jumps > 128, "{} jumps", run.output.jumps);
        assert_finished(&run);
        assert_eq!(run.get("N"), 300);
    }

    #[test]
    fn subroutine_and_control_errors() {
        let cases = [
            ("FUNCTION F(N)\nF = F(N - 1)\nEND FUNCTION", "recursion is not supported"),
            ("SUB A\nB\nEND SUB\nSUB B\nA\nEND SUB", "recursion is not supported"),
            ("SUB S\nEND SUB\nX = S", "is a SUB and has no value"),
            ("SUB S\nEND SUB\nX = S(1)", "only a FUNCTION can be used"),
            ("FUNCTION F(A)\nEND FUNCTION\nX = F", "call it with its arguments"),
            ("FUNCTION F(A, B)\nEND FUNCTION\nX = F(1)", "expects 2 argument(s)"),
            ("SUB S\nSUB T\nEND SUB", "cannot be nested"),
            ("FOR I = 1 TO 2\nSUB S\nEND SUB\nNEXT", "cannot be inside a block"),
            ("SUB MIN\nEND SUB", "built-in function"),
            ("SUB S\nEND SUB\nSUB S\nEND SUB", "defined twice"),
            ("FOO 1", "unknown SUB FOO"),
            ("EXIT FOR", "EXIT FOR outside of FOR"),
            ("WHILE 1\nSUB S\nEXIT WHILE\nEND SUB\nWEND", "cannot be inside a block"),
            ("CASE 1", "CASE without SELECT CASE"),
            ("SELECT CASE 1\nCASE ELSE\nCASE 2\nEND SELECT", "CASE after CASE ELSE"),
            ("DO\nX = 1", "DO without LOOP"),
            ("GOTO NOWHERE", "label NOWHERE does not exist"),
            ("A:\nA:", "duplicate label A"),
            ("FOR I = 1 TO 2 STEP 0\nNEXT", "STEP must not be 0"),
            ("IF \"a\" = 1 THEN X = 1", "cannot compare a string with a number"),
            ("X = RND(1)", "RND takes"),
            ("SHARED X", "SHARED outside"),
            ("REQ math, joystick", "requires the joystick interrupt, which is not available (available: proc, math, random, surface, string, trig)"),
        ];
        for (src, msg) in cases {
            match build(src) {
                Ok(_) => panic!("{:?} compiled", src),
                Err(e) => assert!(e.msg.contains(msg), "{:?}: {}", src, e.msg)
            }
        }
        assert!(build("X = 1 : STOP").unwrap().listing.contains("brk"));
        assert!(build("REQ math, surface\nREQ STRING\nX = 1").is_ok());
        let e = compile("test", "X = 1\nREQ math", &[], &Functions::new()).err().unwrap();
        assert_eq!(e.line, 2);
        assert!(e.msg.contains("(available: none)"), "{}", e.msg);
    }

    #[test]
    fn detection() {
        use virtmach::basic::is_basic;
        for src in ["REM x\nX = 1", "\n\n  rem lower case", "10 REM numbered", "REM", "'x\nREM"] {
            assert_eq!(is_basic(src), !src.starts_with('\''), "{:?}", src);
        }
        for src in ["; listing\nREM", "reg r0", "REMARK = 1", "", "#req math", "X = 1"] {
            assert!(!is_basic(src), "{:?}", src);
        }
    }

    #[test]
    fn integrated_compile() {
        let names: Vec<String> = INTERRUPTS.iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let basic = std::fs::read_to_string(programs_dir().join("ci/gcd.bas")).unwrap();
        let listing = compile("gcd", &basic, &interrupts, &functions).unwrap().listing;

        let from_basic = VirtMach::compile_owned("gcd", &basic, &interrupts, functions.clone()).unwrap().0;
        let from_listing = VirtMach::compile_owned("gcd", &listing, &interrupts, functions.clone()).unwrap().0;
        assert_eq!(from_basic.data, from_listing.data);

        match VirtMach::compile_owned("bad", "REM broken\nX = math.and(1)", &interrupts, functions.clone()) {
            Err(ListingError::Basic(2, msg)) => assert!(msg.contains("expects 2 argument(s)"), "{}", msg),
            other => panic!("{:?}", other.map(|_| ()))
        }
        assert!(matches!(VirtMach::compile_owned("listing", "X = 1", &interrupts, functions), Err(ListingError::IllegalOp(1, _))));
    }

    #[test]
    fn conditions() {
        let run = run_src("
            A = 5 : B = 7 : R = 0
            IF A < B THEN R = R + 1
            IF A > B THEN R = R + 100
            IF A <= 5 AND B >= 7 THEN R = R + 2
            IF A = 4 OR B <> 7 THEN R = R + 100 ELSE R = R + 4
            IF NOT A = B THEN R = R + 8
            IF A - 5 THEN R = R + 100
            IF B THEN
              R = R + 16
            ELSE
              R = R + 100
            END IF
            V = ABS(-9) + ABS(3)
            N = -A
            END", 0);
        assert_finished(&run);
        assert_eq!(run.get("R"), 31);
        assert_eq!(run.get("V"), 12);
        assert_eq!(run.get("N"), -5);
    }

    #[test]
    fn peripherals() {
        // only compiled and assembled, running them needs the simulators of examples/helpers.rs
        let names: Vec<String> = ["proc", "math", "string", "time", "gpio", "uart", "i2c", "pwm"].iter().map(|s| s.to_string()).collect();
        let (interrupts, functions) = load_interrupts(&include_dirs(), &names).unwrap();
        let files: Vec<PathBuf> = std::fs::read_dir(programs_dir().join("peripherals")).unwrap()
            .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e| e == "bas")).collect();
        assert!(files.len() >= 5, "{:?}", files);
        for file in files {
            let src = std::fs::read_to_string(&file).unwrap();
            let output = compile("test", &src, &interrupts, &functions).unwrap_or_else(|e| panic!("{}: line {}: {}", file.display(), e.line, e.msg));
            VirtMach::compile_owned("test", &output.listing, &interrupts, functions.clone())
                .unwrap_or_else(|e| panic!("{}: {:?}\n{}", file.display(), e, output.listing));
        }
    }

    #[test]
    fn errors() {
        let cases = [
            ("X = foo.bar(1)", "unknown interrupt function"),
            ("X = math.foo(1)", "has no function"),
            ("X = math.and(1)", "expects 2 argument(s)"),
            ("surface.clear()", "expects 1 argument(s)"),
            ("X = surface.get_size()", "cannot be used in an expression"),
            ("FOR I = 1 TO 3\nNEXT J", "does not match"),
            ("GOTO 100", "does not exist"),
            ("WHILE 1", "without WEND"),
            ("X = Y(1)", "unknown function"),
            ("DIM A(3)\nA(4) = 1", "out of bounds"),
            ("X = 40000", "out of range"),
            ("X = = 1", "syntax error"),
        ];
        for (src, msg) in cases {
            match build(src) {
                Ok(_) => panic!("{:?} compiled", src),
                Err(e) => assert!(e.msg.contains(msg), "{:?}: {}", src, e.msg)
            }
        }
        let e = compile("test", "X = 2 * 3", &[], &Functions::new()).err().unwrap();
        assert!(e.msg.contains("add math to the interrupts"), "{}", e.msg);
    }
}
