use std::{thread, time};
use virtmach::VirtMach;
use virtmach::interrupts::{SoftInterrupt, Proc, Math, Random, BASE_INTERRUPTS_DEFS};

mod helpers;

fn main(){
    match helpers::load_file("examples/programs/count.txt") {
        Ok(content) => {            
            match VirtMach::compile(content.0.as_str(), content.1.as_str(), BASE_INTERRUPTS_DEFS) {
                Ok(res) => {                    
                    let program = res.0;

                    let mut vm = VirtMach::new();          

                    vm.load_program(program);

                    let interrupts: &mut [&mut dyn SoftInterrupt] = &mut [ &mut Proc {}, &mut Math {}, &mut Random {}];

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