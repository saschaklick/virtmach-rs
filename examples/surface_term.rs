#![allow(static_mut_refs)]

use std::{thread, time};
use virtmach::{Runtime, VirtMach};
use virtmach::{ RuntimeError, interrupt_set };
use bitmap_writer::{Bitmap, Writer, Frame, Style};

mod helpers;

#[path = "../sdk/src/lib/int_surface_term.rs"]
mod int_surface_term;

const W: usize = 64;
const H: usize = 40;

static mut BUF: [u8;W * H / 8] = [0b00000000;W * H / 8];           

// The interrupts once, for the tables to compile with and for the vm to run with. The `_` ones
// are the variables of main, given where the list is made.
interrupt_set!(interrupts = [math, proc, string, time => _, periph => _, surface => _]);

fn main(){
    let tables = interrupts!(tables);

    let Some(source) = helpers::load_source("examples/programs/gfx/starfield.txt", &tables) else { return };

    match VirtMach::compile_tables(source.name.as_str(), source.code.as_str(), &tables) {
        Ok(res) => {                    
            let program = res.0;

            let mut vm = VirtMach::new();

            vm.load_program(program);

            let mut w = Writer::new();
            w.frame(Frame::UnicodeDoubleUFrame)
            .style(Style::UnicodeBlock1x2)
            .ansi_position(1, 1);                    
                                                        

            let mut time = helpers::SimTime::default();
            let mut hw = helpers::SimPeripherals::default();

            loop {
                let ready = time.ready();
                let mut surface = int_surface_term::IntSurface { w: W as i32, h: H as i32, clip: [0, 0, W as i32, H as i32 ], bitmap: unsafe { &mut BUF } };
                interrupts!(let list, &mut time, &mut hw, &mut surface);

                if ready { vm.run(4096, list); time.after_run(&vm); }

                let mut dashboard = String::new();
                vm.write_dashboard(&mut dashboard, 0b111, 6);
                
                if vm.error == RuntimeError::NoError {                                    
                    if vm.state != Runtime::Run {
                        print!("\x1b[J");
                        let bitmap = Bitmap::new(W, H, unsafe { &mut BUF });                                   
                        w.print(&bitmap);
                    
                        for (i, line) in dashboard.lines().enumerate() { print!("\x1b[{};{}H {}\x1b[K", i + 1, W + 3, line); }
                        helpers::print_variables_beside(&vm, &source.variables, &dashboard, 1, W + 4);
                        // below the framed bitmap, 2 pixel rows per line
                        let row = helpers::report_row(&dashboard, &source.variables, 1).max(H / 2 + 4);
                        helpers::print_hardware_report(&hw, row, 1);
                    }
                } else {                                    
                    println!("{}", dashboard);
                    break;
                }                                

                thread::sleep(time::Duration::from_millis(1000 / 15))
            }
            
        }
        Err(err) => println!("compile error: {:?}", err)
    }
}