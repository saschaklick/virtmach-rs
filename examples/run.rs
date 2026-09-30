use std::{env, thread, time};
use virtmach::VirtMach;
use virtmach::interrupts::{ self, SoftInterrupt, SoftInterruptFunction };

mod helpers;

fn main(){
    let delay = env::args().nth(2).and_then(|ms| ms.parse::<u64>().ok()).unwrap_or(250);

    let tables: Vec<(&str, &[SoftInterruptFunction])> = [
        (interrupts::proc::NAME, interrupts::proc::FUNCTIONS.as_slice()),
        (interrupts::math::NAME, interrupts::math::FUNCTIONS.as_slice()),
        (interrupts::random::NAME, interrupts::random::FUNCTIONS.as_slice()),
        (interrupts::trig::NAME, interrupts::trig::FUNCTIONS.as_slice())
    ].to_vec();

    let Some(source) = helpers::load_source("examples/programs/count.txt", &tables) else { return };

    match VirtMach::compile(source.name.as_str(), source.code.as_str(), tables) {
        Ok(res) => {
            let program = res.0;

            let mut vm = VirtMach::new();

            vm.load_program(program);

            let interrupts: &mut [&mut dyn SoftInterrupt] = &mut [
                &mut interrupts::proc::Interrupt {},
                &mut interrupts::math::Interrupt {},
                &mut interrupts::random::Interrupt {},
                &mut interrupts::trig::Interrupt {}
            ];

            loop {
                vm.run(1, interrupts);

                let mut dashboard = String::new();
                vm.write_dashboard(&mut dashboard, 0b111, 5);

                print!("\x1b[H\x1b[J");
                print!("{}", dashboard);
                helpers::print_variables_beside(&vm, &source.variables, &dashboard, 1, 1);
                println!();

                thread::sleep(time::Duration::from_millis(delay))
            }
        }
        Err(err) => println!("compile error: {:?}", err)
    }
}
