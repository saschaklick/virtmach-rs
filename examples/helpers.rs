use std::{ collections::VecDeque, env, ffi::OsStr, fs::File, io::{ Read, Write }, ops::Range, path::Path, time::{ Duration, Instant, SystemTime, UNIX_EPOCH } };
use virtmach::{ VirtMach, VMAtom, Program, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptFunction, gpio, uart, i2c, time, pwm } };

#[allow(dead_code)]
pub fn disassemble(program: Program) {
    println!();
    println!("Program \"{}\" ({}b):", program.id, program.data.len());
    println!();
    let mut pos = 0usize;
    let instructions = program.get_instructions();
    loop {
        let mut op = String::new();
        let addr = pos;                                        
        pos = VirtMach::decompile(&program, pos, &mut op);
        let slice = &instructions[addr..pos];
        let hex_wid = (1 + size_of::<VMAtom>()) * 3 - 1;
        println!("\t{:04x} | {:hex_wid$} | {}", addr, slice.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "), op.as_str());        

        if pos >= instructions.len() { break }
    }       
    println!();
}

#[allow(dead_code)]
pub fn load_file(filename: &str) -> Result<(String, String), std::io::Error> {
    match File::open(filename) {
        Ok(mut file) => {
            let name = String::from(Path::new(filename).file_stem().unwrap_or(OsStr::new("n/a")).to_str().unwrap_or("n/a"));
            let mut content = String::new();
            match file.read_to_string(&mut content) {
                Ok(_) => Ok((name, content)),                
                Err(err) => Err(err)
            }
        }
        Err(err) => Err(err)
    }
}

/// Where a BASIC variable is kept in the vm
#[allow(dead_code)]
pub enum Slot { Reg(usize), Mem(usize) }

/// Source for VirtMach::compile, a listing or (with the basic feature) a BASIC program starting with REM,
/// plus the BASIC variables for displaying their values
#[allow(dead_code)]
pub struct Source {
    pub filename: String,
    pub name: String,
    pub code: String,
    pub variables: Vec<(String, Slot)>
}

/// Loads the file given as first command line argument, or `default`. Errors are printed and return None.
#[allow(dead_code)]
pub fn load_source(default: &str, tables: &[(&str, &[SoftInterruptFunction])]) -> Option<Source> {
    let filename = env::args().nth(1).unwrap_or(String::from(default));
    match load_file(&filename) {
        Ok((name, code)) => {
            let variables = basic_variables(&name, &code, tables)?;
            Some(Source { filename, name, code, variables })
        }
        Err(err) => { println!("file read error: {}: {:?}", filename, err); None }
    }
}

/// Variables of a BASIC program, empty for listings. BASIC errors are printed and return None.
#[cfg(feature = "basic")]
fn basic_variables(name: &str, code: &str, tables: &[(&str, &[SoftInterruptFunction])]) -> Option<Vec<(String, Slot)>> {
    use virtmach::basic::{ self, Loc };

    if !basic::is_basic(code) { return Some(vec![]); }
    let (interrupts, functions) = basic::functions(tables);
    match basic::compile(name, code, &interrupts, &functions) {
        Ok(output) => {
            let mut variables: Vec<(String, Slot)> = output.variables.iter()
                .filter(|(name, _)| !name.starts_with('_'))
                .map(|(name, loc)| (name.clone(), match loc { Loc::Reg(r) => Slot::Reg(*r as usize), Loc::Mem(a) => Slot::Mem(*a) }))
                .collect();
            for (array, base, size) in &output.arrays {
                variables.extend((0..*size).map(|i| (format!("{}({})", array, i), Slot::Mem(base + i))));
            }
            Some(variables)
        }
        Err(err) => { println!("BASIC error in line {}: {}", err.line, err.msg); None }
    }
}

#[cfg(not(feature = "basic"))]
fn basic_variables(_name: &str, code: &str, _tables: &[(&str, &[SoftInterruptFunction])]) -> Option<Vec<(String, Slot)>> {
    let first = code.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if first.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start().to_ascii_uppercase().starts_with("REM") {
        println!("this looks like a BASIC program, compiling it needs the basic feature: cargo run --example <example> --features basic -- <file>");
        return None;
    }
    Some(vec![])
}

/// Where each BASIC variable is kept, e.g. "SUM        r0"
#[allow(dead_code)]
pub fn variable_locations(variables: &[(String, Slot)]) -> Vec<String> {
    variables.iter().map(|(name, slot)| match slot {
        Slot::Reg(r) => format!("{:>10} r{}", name, r),
        Slot::Mem(a) => format!("{:>10} memory #{}", name, a)
    }).collect()
}

/// Current value of each BASIC variable, e.g. "SUM = 55"
#[allow(dead_code)]
pub fn variable_values(vm: &VirtMach, variables: &[(String, Slot)]) -> Vec<String> {
    variables.iter().map(|(name, slot)| {
        let value = match slot { Slot::Reg(r) => vm.registers[*r], Slot::Mem(a) => vm.memory[*a] };
        format!("{:>10} = {}", name, value)
    }).collect()
}

/// Prints the BASIC variable values with ANSI cursor positioning in a column to the right of the
/// dashboard, which was printed at the given 1-based row and column. Leaves the cursor at the start
/// of the screen line below the dashboard.
#[allow(dead_code)]
pub fn print_variables_beside(vm: &VirtMach, variables: &[(String, Slot)], dashboard: &str, row: usize, column: usize) {
    let column = column + dashboard.lines().map(|l| l.chars().count()).max().unwrap_or(0) + 1;
    let values: Vec<String> = variable_values(vm, variables).iter().map(|l| format!("|{}", l)).collect();
    if !values.is_empty() {
        let width = values.iter().map(|l| l.chars().count()).max().unwrap_or(0).max(16);
        let header = [String::from("BASIC VARIABLES"), "-".repeat(width)];
        for (i, line) in header.iter().chain(values.iter()).enumerate() { print!("\x1b[{};{}H{}\x1b[K", row + i, column, line); }
    }
    print!("\x1b[{};1H", row + dashboard.lines().count());
    let _ = std::io::stdout().flush();
}

/// Pops the arguments of interrupt function op as listed in its table, None if op is not in it
#[allow(dead_code)]
fn pop_arguments(vm: &mut VirtMach, functions: &[SoftInterruptFunction], op: VMAtom) -> Option<Vec<VMAtom>> {
    let function = functions.iter().find(|f| f.no == op)?;
    Some((0..function.arguments).map(|_| vm.stack_pop()).collect())
}

/// The memory cells from address on, sets RuntimeError::MemoryOutOfBounds if they do not fit
#[allow(dead_code)]
fn memory_range(vm: &mut VirtMach, address: VMAtom, length: VMAtom) -> Option<Range<usize>> {
    let range = usize::try_from(address).ok().zip(usize::try_from(length).ok()).map(|(a, l)| a..a + l);
    match range {
        Some(range) if range.end <= vm.memory.len() => Some(range),
        _ => { vm.error = RuntimeError::MemoryOutOfBounds; None }
    }
}

/// length bytes of a string from start, as far as the string reaches
#[allow(dead_code)]
fn string_part(vm: &mut VirtMach, index: VMAtom, start: VMAtom, length: VMAtom) -> Vec<u8> {
    let bin = vm.get_bin(index as u8);
    let start = (start.max(0) as usize).min(bin.len());
    let end = start.saturating_add(length.max(0) as usize).min(bin.len());
    bin[start..end].to_vec()
}

#[allow(dead_code)]
fn count(n: usize) -> VMAtom { n.min(VMAtom::MAX as usize) as VMAtom }

pub const SIM_PINS: usize = 8;

/// A pin of SimGpio. mode and pull are numbered like in the gpio interrupt, input is the level
/// something outside drives the pin to, None if nothing does.
#[allow(dead_code)]
#[derive(Clone, Copy, Default)]
pub struct SimPin {
    pub mode: VMAtom,
    pub pull: VMAtom,
    pub output: bool,
    pub input: Option<bool>
}

impl SimPin {
    /// The level the pin reads: an output its own, an open-drain output pulls it low or lets it
    /// float like an input, then the outside level wins over the pull resistor, floating is low
    pub fn level(&self) -> bool {
        match self.mode {
            1 => self.output,
            2 if !self.output => false,
            _ => self.input.unwrap_or(self.pull == 1)
        }
    }
}

/// The gpio interrupt on SIM_PINS simulated pins, see the gpio template for the functions.
/// Invalid pins, ports, modes and pulls set RuntimeError::InterruptError.
#[allow(dead_code)]
#[derive(Default)]
pub struct SimGpio {
    /// whether the program called a gpio function, the report shows the pins from then on
    pub called: bool,
    pub pins: [SimPin; SIM_PINS]
}

#[allow(dead_code)]
impl SimGpio {
    fn pin(&mut self, vm: &mut VirtMach, pin: VMAtom) -> Option<&mut SimPin> {
        let found = usize::try_from(pin).ok().and_then(|p| self.pins.get_mut(p));
        if found.is_none() { vm.error = RuntimeError::InterruptError; }
        found
    }

    /// The pins of a port with their bit in the mask
    fn port(&mut self, vm: &mut VirtMach, port: VMAtom) -> Option<impl Iterator<Item = (u32, &mut SimPin)>> {
        let first = usize::try_from(port).ok().map(|p| p * VMAtom::BITS as usize).filter(|p| *p < SIM_PINS);
        let Some(first) = first else { vm.error = RuntimeError::InterruptError; return None };
        Some(self.pins[first..].iter_mut().take(VMAtom::BITS as usize).enumerate().map(|(n, pin)| (n as u32, pin)))
    }
}

impl SoftInterrupt for SimGpio {
    fn name(&self) -> &str {
        return gpio::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] {
        return &gpio::FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        self.called = true;
        let op = vm.stack_pop();
        let Some(args) = pop_arguments(vm, &gpio::FUNCTIONS, op) else { vm.error = RuntimeError::UnimplementedInterruptFunc; return };
        match op {
            0 | 1 if !(0..=2).contains(&args[1]) => { vm.error = RuntimeError::InterruptError; }
            0 => { if let Some(pin) = self.pin(vm, args[0]) { pin.mode = args[1]; } }
            1 => { if let Some(pin) = self.pin(vm, args[0]) { pin.pull = args[1]; } }
            2 => { if let Some(pin) = self.pin(vm, args[0]) { pin.output = args[1] != 0; } }
            3 => { if let Some(pin) = self.pin(vm, args[0]) { pin.output = true; } }
            4 => { if let Some(pin) = self.pin(vm, args[0]) { pin.output = false; } }
            5 => { if let Some(pin) = self.pin(vm, args[0]) { pin.output = !pin.output; } }
            6 => {
                let level = self.pin(vm, args[0]).is_some_and(|pin| pin.level());
                vm.stack_push(level as VMAtom);
            }
            10 => {
                let (mask, values) = (args[1], args[2]);
                for (bit, pin) in self.port(vm, args[0]).into_iter().flatten() {
                    if mask >> bit & 1 != 0 { pin.output = values >> bit & 1 != 0; }
                }
            }
            11 => {
                let mask = args[1];
                let values = self.port(vm, args[0]).into_iter().flatten()
                    .fold(0 as VMAtom, |values, (bit, pin)| values | ((pin.level() as VMAtom) << bit));
                vm.stack_push(values & mask);
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

/// How many bytes SimUart keeps in sent and received
pub const SIM_UART_LOG: usize = 8;

/// The uart interrupt with port 0 only, other ports set RuntimeError::InterruptError. Bytes
/// given to receive wait in rx to be read, with loopback the sent bytes are received too.
/// sent and received keep the last SIM_UART_LOG bytes of each direction for the report.
#[allow(dead_code)]
#[derive(Default)]
pub struct SimUart {
    /// whether the program called a uart function, the report shows the uart from then on
    pub called: bool,
    pub loopback: bool,
    pub rx: VecDeque<u8>,
    pub sent: VecDeque<u8>,
    pub received: VecDeque<u8>
}

#[allow(dead_code)]
impl SimUart {
    /// Bytes arriving from the outside
    pub fn receive(&mut self, bytes: &[u8]) {
        self.rx.extend(bytes);
        for byte in bytes { log_byte(&mut self.received, *byte); }
    }

    fn send(&mut self, bytes: &[u8]) -> VMAtom {
        for byte in bytes { log_byte(&mut self.sent, *byte); }
        if self.loopback { self.receive(bytes); }
        count(bytes.len())
    }

    fn read(&mut self, max: VMAtom) -> Vec<u8> {
        let n = (max.max(0) as usize).min(self.rx.len());
        self.rx.drain(..n).collect()
    }
}

#[allow(dead_code)]
fn log_byte(log: &mut VecDeque<u8>, byte: u8) {
    if log.len() == SIM_UART_LOG { log.pop_front(); }
    log.push_back(byte);
}

impl SoftInterrupt for SimUart {
    fn name(&self) -> &str {
        return uart::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] {
        return &uart::FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        self.called = true;
        let op = vm.stack_pop();
        let Some(args) = pop_arguments(vm, &uart::FUNCTIONS, op) else { vm.error = RuntimeError::UnimplementedInterruptFunc; return };
        if args[0] != 0 {
            vm.error = RuntimeError::InterruptError;
            match op { 2 => vm.stack_push(-1), 1 | 4 | 10..=13 => vm.stack_push(0), _ => {} }
            return;
        }
        match op {
            0 => {}
            1 => { vm.stack_push(count(self.rx.len())); }
            2 => { let byte = self.rx.pop_front().map_or(-1, |b| b as VMAtom); vm.stack_push(byte); }
            3 => { self.send(&[args[1] as u8]); }
            4 => { vm.stack_push(SIM_UART_LOG as VMAtom); }
            10 => {
                let bytes = self.read(args[2]);
                vm.set_bin(args[1] as u8, &bytes);
                vm.stack_push(count(bytes.len()));
            }
            11 => {
                let n = (args[2].max(0) as usize).min(self.rx.len()) as VMAtom;
                let read = match memory_range(vm, args[1], n) {
                    Some(range) => { let bytes = self.read(n); vm.memory[range].iter_mut().zip(&bytes).for_each(|(cell, b)| *cell = *b as VMAtom); n }
                    None => 0
                };
                vm.stack_push(read);
            }
            12 => {
                let bytes = string_part(vm, args[1], args[2], args[3]);
                let sent = self.send(&bytes);
                vm.stack_push(sent);
            }
            13 => {
                let bytes: Vec<u8> = memory_range(vm, args[1], args[2]).map(|r| vm.memory[r].iter().map(|v| *v as u8).collect()).unwrap_or_default();
                let sent = self.send(&bytes);
                vm.stack_push(sent);
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

/// How many registers the SimI2c device has
pub const SIM_I2C_REGS: usize = 8;

/// The i2c interrupt with port 0 only, other ports set RuntimeError::InterruptError. On the bus is
/// one device at address with SIM_I2C_REGS registers, like most sensors and EEPROMs: the first byte
/// of a write sets the register pointer, the others are written from there, reads start at the
/// pointer, which moves on with each byte and wraps around. Other addresses do not acknowledge.
#[allow(dead_code)]
pub struct SimI2c {
    /// whether the program called an i2c function, the report shows the i2c device from then on
    pub called: bool,
    pub address: VMAtom,
    pub registers: [u8; SIM_I2C_REGS],
    pub pointer: usize
}

impl Default for SimI2c {
    fn default() -> Self {
        Self { called: false, address: 0x50, registers: [0; SIM_I2C_REGS], pointer: 0 }
    }
}

#[allow(dead_code)]
impl SimI2c {
    fn write(&mut self, bytes: &[u8]) -> VMAtom {
        if let Some((reg, data)) = bytes.split_first() {
            self.pointer = *reg as usize % SIM_I2C_REGS;
            for byte in data {
                self.registers[self.pointer] = *byte;
                self.pointer = (self.pointer + 1) % SIM_I2C_REGS;
            }
        }
        count(bytes.len())
    }

    fn read(&mut self, length: VMAtom) -> Vec<u8> {
        (0..length.max(0)).map(|_| {
            let byte = self.registers[self.pointer];
            self.pointer = (self.pointer + 1) % SIM_I2C_REGS;
            byte
        }).collect()
    }

    fn read_into_memory(&mut self, vm: &mut VirtMach, address: VMAtom, length: VMAtom) -> VMAtom {
        let Some(range) = memory_range(vm, address, length) else { return 0 };
        let bytes = self.read(length);
        vm.memory[range].iter_mut().zip(&bytes).for_each(|(cell, b)| *cell = *b as VMAtom);
        length
    }
}

impl SoftInterrupt for SimI2c {
    fn name(&self) -> &str {
        return i2c::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] {
        return &i2c::FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        self.called = true;
        let op = vm.stack_pop();
        let Some(args) = pop_arguments(vm, &i2c::FUNCTIONS, op) else { vm.error = RuntimeError::UnimplementedInterruptFunc; return };
        if args[0] != 0 {
            vm.error = RuntimeError::InterruptError;
            if op != 0 { vm.stack_push(0); }
            return;
        }
        if op == 0 { return; }
        // no acknowledge
        if args[1] != self.address { vm.stack_push(-1); return; }
        let result = match op {
            1 => 0,
            2 => self.write(&[args[2] as u8]),
            3 => self.read(1)[0] as VMAtom,
            4 => match usize::try_from(args[2]).ok().filter(|r| *r < SIM_I2C_REGS) {
                Some(reg) => { self.registers[reg] = args[3] as u8; 1 }
                None => -2
            },
            5 => match usize::try_from(args[2]).ok().filter(|r| *r < SIM_I2C_REGS) {
                Some(reg) => self.registers[reg] as VMAtom,
                None => -2
            },
            10 => {
                let bytes: Vec<u8> = memory_range(vm, args[2], args[3]).map(|r| vm.memory[r].iter().map(|v| *v as u8).collect()).unwrap_or_default();
                self.write(&bytes)
            }
            11 => self.read_into_memory(vm, args[2], args[3]),
            12 => { let bytes = string_part(vm, args[2], args[3], args[4]); self.write(&bytes) }
            13 => { let bytes = self.read(args[3]); vm.set_bin(args[2] as u8, &bytes); count(bytes.len()) }
            14 => {
                let bytes: Vec<u8> = memory_range(vm, args[2], args[3]).map(|r| vm.memory[r].iter().map(|v| *v as u8).collect()).unwrap_or_default();
                self.write(&bytes);
                self.read_into_memory(vm, args[4], args[5])
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; 0 }
        };
        vm.stack_push(result);
    }
}

/// The time interrupt on the host clock, see the time template for the functions. Waiting does not
/// block: it pauses the vm until the deadline, so the example only runs it again once ready()
/// is true. get_time and get_date are UTC, year needs at least 16 bit atoms. Negative intervals
/// set RuntimeError::InterruptError.
#[allow(dead_code)]
pub struct SimTime {
    pub deadline: Option<Instant>,
    /// when the last wait_until returned, the first one counts from the start
    pub last: Instant
}

impl Default for SimTime {
    fn default() -> Self {
        Self { deadline: None, last: Instant::now() }
    }
}

#[allow(dead_code)]
impl SimTime {
    /// Whether the vm may run, false while a wait has not reached its deadline
    pub fn ready(&mut self) -> bool {
        if self.deadline.is_some_and(|deadline| Instant::now() < deadline) { return false; }
        self.deadline = None;
        true
    }

    fn wait(&mut self, vm: &mut VirtMach, deadline: Instant) {
        self.deadline = Some(deadline);
        vm.pause();
    }
}

/// seconds * 1000 + milliseconds, None if negative
#[allow(dead_code)]
fn interval(seconds: VMAtom, milliseconds: VMAtom) -> Option<Duration> {
    let (seconds, milliseconds) = (u64::try_from(seconds).ok()?, u64::try_from(milliseconds).ok()?);
    Some(Duration::from_secs(seconds) + Duration::from_millis(milliseconds))
}

/// Year, month 1-12 and day 1-31 of days since 1970-01-01, from Howard Hinnant's civil_from_days
#[allow(dead_code)]
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (month <= 2) as i64, month, day)
}

impl SoftInterrupt for SimTime {
    fn name(&self) -> &str {
        return time::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] {
        return &time::FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        let op = vm.stack_pop();
        let Some(args) = pop_arguments(vm, &time::FUNCTIONS, op) else { vm.error = RuntimeError::UnimplementedInterruptFunc; return };
        let now = Instant::now();
        let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        match op {
            0 | 1 => {
                let Some(interval) = interval(args[0], args[1]) else {
                    vm.error = RuntimeError::InterruptError;
                    if op == 1 { vm.stack_push(0); }
                    return;
                };
                if op == 0 { self.wait(vm, now + interval); return; }
                // too late: return at once and count the next interval from now
                let target = self.last + interval;
                let waited = target > now;
                self.last = if waited { target } else { now };
                vm.stack_push(waited as VMAtom);
                if waited { self.wait(vm, target); }
            }
            2 => {
                let ms = since_epoch.as_millis() % 86_400_000;
                for value in [ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000] { vm.stack_push(value as VMAtom); }
            }
            3 => {
                let (year, month, day) = civil_from_days((since_epoch.as_secs() / 86_400) as i64);
                for value in [year, month, day] { vm.stack_push(value as VMAtom); }
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

/// A PWM output of SimPwm, duty is 0.0 to 1.0
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct SimPwmOutput {
    pub frequency: VMAtom,
    pub duty: f32
}

/// The pwm interrupt on the SIM_PINS pins of SimGpio, see the pwm template for the functions.
/// Every pin can output PWM at any frequency from 1 Hz up, so setup returns the frequency it was
/// given, or 0 for frequencies below 1. While a pin outputs PWM the report shows it instead of
/// the gpio state, after stop the gpio state again. Invalid pins, negative values, a max of 0 and
/// pins that were not set up set RuntimeError::InterruptError.
#[allow(dead_code)]
#[derive(Default)]
pub struct SimPwm {
    /// whether the program called a pwm function, the report shows the pins from then on
    pub called: bool,
    pub outputs: [Option<SimPwmOutput>; SIM_PINS]
}

impl SoftInterrupt for SimPwm {
    fn name(&self) -> &str {
        return pwm::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] {
        return &pwm::FUNCTIONS;
    }

    fn call(&mut self, vm: &mut VirtMach) {
        self.called = true;
        let op = vm.stack_pop();
        let Some(args) = pop_arguments(vm, &pwm::FUNCTIONS, op) else { vm.error = RuntimeError::UnimplementedInterruptFunc; return };
        let Some(output) = usize::try_from(args[0]).ok().and_then(|p| self.outputs.get_mut(p)) else {
            vm.error = RuntimeError::InterruptError;
            if op == 0 { vm.stack_push(0); }
            return;
        };
        match op {
            0 => {
                let frequency = args[1];
                if frequency > 0 { *output = Some(SimPwmOutput { frequency, duty: 0.0 }); }
                vm.stack_push(frequency.max(0));
            }
            1 | 2 => {
                let duty = match (op, *output) {
                    (1, Some(_)) if args[1] >= 0 && args[2] > 0 => Some(args[1].min(args[2]) as f32 / args[2] as f32),
                    (2, Some(out)) if args[1] >= 0 => Some((args[1] as f32 * out.frequency as f32 / 1_000_000.0).min(1.0)),
                    _ => None
                };
                match (duty, output) {
                    (Some(duty), Some(out)) => { out.duty = duty; }
                    _ => { vm.error = RuntimeError::InterruptError; }
                }
            }
            3 => { *output = None; }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }
    }
}

/// The simulated peripherals for the report, the interrupt arrays take each one by &mut
#[allow(dead_code)]
pub struct SimPeripherals {
    pub gpio: SimGpio,
    pub uart: SimUart,
    pub i2c: SimI2c,
    pub pwm: SimPwm
}

impl Default for SimPeripherals {
    /// with the uart in loopback, so programs receive what they send
    fn default() -> Self {
        Self { gpio: SimGpio::default(), uart: SimUart { loopback: true, ..Default::default() }, i2c: SimI2c::default(), pwm: SimPwm::default() }
    }
}

/// The state of the simulated hardware under a PERIPHERALS header, framed like the BASIC
/// variables, as lines of 2 character columns divided by "|". Only the peripherals the program
/// called show up, no lines at all before it called any:
/// - gpio or pwm: pin numbers, modes (I input, O output, o open-drain, U pull-up, D pull-down,
///   P PWM) and levels (X high, . low, ~ PWM), with pwm also the duty cycles, 00 to ff
/// - uart: the last bytes sent (>>) and received (<<)
/// - i2c: the registers, with the address of the device below I2C
#[allow(dead_code)]
pub fn hardware_report(hw: &SimPeripherals) -> Vec<String> {
    let row = |label: &str, cells: Vec<String>| format!("{:<8}{}", label, cells.iter().map(|c| format!("{:>2}", c)).collect::<Vec<_>>().join("|"));
    let bytes = |log: &VecDeque<u8>| (0..SIM_UART_LOG).map(|i| log.get(i).map_or(String::new(), |b| format!("{:02x}", b))).collect();
    let pins = || hw.gpio.pins.iter().zip(&hw.pwm.outputs);
    let mut lines = Vec::new();
    if hw.gpio.called || hw.pwm.called {
        lines.push(row("Pins", (0..SIM_PINS).map(|i| i.to_string()).collect()));
        lines.push(row("", pins().map(|(pin, out)| match out {
            Some(_) => String::from("P"),
            None => format!("{}{}", ["I", "O", "o"][pin.mode as usize], ["", "U", "D"][pin.pull as usize])
        }).collect()));
        lines.push(row("", pins().map(|(pin, out)| String::from(match out { Some(_) => "~", None if pin.level() => "X", None => "." })).collect()));
    }
    if hw.pwm.called {
        lines.push(row("PWM", hw.pwm.outputs.iter().map(|out| out.map_or(String::new(), |o| format!("{:02x}", (o.duty * 255.0).round() as u8))).collect()));
    }
    if hw.uart.called {
        lines.push(row("UART >>", bytes(&hw.uart.sent)));
        lines.push(row("     <<", bytes(&hw.uart.received)));
    }
    if hw.i2c.called {
        lines.push(row("I2C", (0..SIM_I2C_REGS).map(|i| format!("R{}", i)).collect()));
        lines.push(row(&format!("0x{:02x}", hw.i2c.address), hw.i2c.registers.iter().map(|r| format!("{:02x}", r)).collect()));
    }
    if lines.is_empty() { return lines; }
    // framed like the BASIC variables
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 1;
    [String::from("PERIPHERALS"), "-".repeat(width)].into_iter().chain(lines.iter().map(|l| format!("|{}", l))).collect()
}

/// Prints the hardware_report with ANSI cursor positioning at the given 1-based row and column,
/// clearing the rest of each line. Leaves the cursor at the start of the screen line below it,
/// prints nothing while the report is empty.
#[allow(dead_code)]
pub fn print_hardware_report(hw: &SimPeripherals, row: usize, column: usize) {
    let lines = hardware_report(hw);
    if lines.is_empty() { return; }
    for (i, line) in lines.iter().enumerate() { print!("\x1b[{};{}H{}\x1b[K", row + i, column, line); }
    print!("\x1b[{};1H", row + lines.len());
    let _ = std::io::stdout().flush();
}

/// The 1-based screen row for the hardware report, one line below the dashboard printed at row
/// and the BASIC variables printed beside it
#[allow(dead_code)]
pub fn report_row(dashboard: &str, variables: &[(String, Slot)], row: usize) -> usize {
    let variable_lines = if variables.is_empty() { 0 } else { variables.len() + 2 };
    row + dashboard.lines().count().max(variable_lines) + 1
}

/// The 1-based screen column for the hardware report, right of the BASIC variables printed beside
/// the dashboard at column. Wide enough for any value, so the report does not move when they change.
#[allow(dead_code)]
pub fn report_column(dashboard: &str, variables: &[(String, Slot)], column: usize) -> usize {
    let column = column + dashboard.lines().map(|l| l.chars().count()).max().unwrap_or(0) + 1;
    if variables.is_empty() { return column; }
    let name_width = variables.iter().map(|(name, _)| name.chars().count()).max().unwrap_or(0).max(10);
    let width = (1 + name_width + 3 + VMAtom::MIN.to_string().len()).max(16);
    column + width + 1
}

#[warn(dead_code)]
pub fn main() {
}