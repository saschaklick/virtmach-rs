#![allow(static_mut_refs)]

use std::{thread, time};
use virtmach::VirtMach;
use virtmach::interrupts::{self, Proc, Math, Random};
use virtmach::{ RuntimeError, interrupts::{SoftInterrupt} };
use bitmap_writer::{Bitmap, Writer, Frame, Style};

mod helpers;

mod int_surface_term;
use int_surface_term::{IntSurface, DEF};

const W: usize = 64;
const H: usize = 40;

static mut BUF: [u8;W * H / 8] = [0b00000000;W * H / 8];           

fn main(){
    match helpers::load_file("examples/programs/starfield.txt") {
        Ok(content) => {
            let mut interrups = [&interrupts::DummyDef;16];                        
            interrups[0..interrupts::BASE_INTERRUPTS_DEFS.len()].copy_from_slice(interrupts::BASE_INTERRUPTS_DEFS);                        
            interrups[interrupts::BASE_INTERRUPTS_DEFS.len()] = &DEF;                                
            
            match VirtMach::compile(content.0.as_str(), content.1.as_str(), &interrups) {
                Ok(res) => {                    
                    let program = res.0;

                    let mut vm = VirtMach::new();

                    vm.load_program(program);

                    let mut w = Writer::new();
                        w.frame(Frame::UnicodeDoubleUFrame)
                        .style(Style::UnicodeBlock1x2)
                        .ansi_position(1, 1);
                                                                
                                
                    let interrupts: &mut [&mut dyn SoftInterrupt] = &mut [ &mut Proc {}, &mut Math {}, &mut Random {}, &mut IntSurface { w: W as i32, h: H as i32, clip: [0, 0, W as i32, H as i32 ], bitmap: unsafe { &mut BUF } }];                            

                    loop {
                        vm.run(1024, interrupts);

                        let mut dashboard = String::new();
                        vm.write_dashboard(&mut dashboard, 0b111, 6);
                        
                        if vm.error == RuntimeError::NoError {                                    
                            print!("\x1b[J");
                            let bitmap = Bitmap::new(W, H, unsafe { &mut BUF });                                   
                            w.print(&bitmap);
                        
                            for (i, line) in dashboard.lines().enumerate() { print!("\x1b[{};{}H {}\x1b[K", i + 1, W + 3, line); }
                            println!("");
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
        Err(err) => println!("file read error: {:?}", err)               
    }
}