use std::{thread, time};
use virtmach::VirtMach;
use virtmach::interrupts::{ self, SoftInterrupt };

mod helpers;

fn main(){    
    match helpers::load_file("examples/programs/count.txt") {
        Ok(content) => {            
            match VirtMach::compile(content.0.as_str(), content.1.as_str(), [
                (interrupts::proc::NAME, interrupts::proc::FUNCTIONS.as_slice()),
                (interrupts::math::NAME, interrupts::math::FUNCTIONS.as_slice()),
                (interrupts::random::NAME, interrupts::random::FUNCTIONS.as_slice())
            ].to_vec()) {
                Ok(res) => {                    
                    let program = res.0;

                    let mut vm = VirtMach::new();          

                    vm.load_program(program);

                    let interrupts: &mut [&mut dyn SoftInterrupt] = &mut [
                        &mut interrupts::proc::Interrupt {},
                        &mut interrupts::math::Interrupt {},
                        &mut interrupts::random::Interrupt {}
                    ];

                    loop {
                        vm.run(1, interrupts);
                        
                        let mut dashboard = String::new();
                        vm.write_dashboard(&mut dashboard, 0b111, 5);

                        print!("\x1b[H\x1b[J");          
                        println!("{}", dashboard);

                        thread::sleep(time::Duration::from_millis(250))
                    }
                    
                }
                Err(err) => println!("compile error: {:?}", err)
            }
        }
        Err(err) =>  println!("file read error: {:?}", err)                
    }
}