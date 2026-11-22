use std::{env, thread, time};
use virtmach::VirtMach;
use virtmach::interrupt_set;

mod helpers;

// The interrupts once, for the tables to compile with and for the vm to run with
interrupt_set!(interrupts = [math, proc, time => _, periph => _]);

fn main(){
    let delay = env::args().nth(2).and_then(|ms| ms.parse::<u64>().ok()).unwrap_or(250);

    let tables = interrupts!(tables);

    let Some(source) = helpers::load_source("examples/programs/ci/count.txt", &tables) else { return };

    match VirtMach::compile_tables(source.name.as_str(), source.code.as_str(), &tables) {
        Ok(res) => {
            let program = res.0;

            let mut vm = VirtMach::new();

            vm.load_program(program);

            let mut time = helpers::SimTime::default();
            let mut hw = helpers::SimPeripherals::default();

            loop {
                let ready = time.ready();
                interrupts!(let list, &mut time, &mut hw);

                if ready { vm.run(1, list); time.after_run(&vm); }

                let mut dashboard = String::new();
                vm.write_dashboard(&mut dashboard, 0b111, 5);

                print!("\x1b[H\x1b[J");
                print!("{}", dashboard);
                helpers::print_variables_beside(&vm, &source.variables, &dashboard, 1, 1);
                helpers::print_hardware_report(&hw, helpers::report_row(&dashboard, &source.variables, 1), 1);

                thread::sleep(time::Duration::from_millis(delay))
            }
        }
        Err(err) => println!("compile error: {:?}", err)
    }
}
