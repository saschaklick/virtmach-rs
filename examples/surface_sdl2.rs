#![allow(static_mut_refs)]

extern crate sdl2;
use sdl2::{ VideoSubsystem, event::Event, keyboard::Keycode, pixels::Color };

use std::{thread, time};
use virtmach::VirtMach;
use virtmach::interrupts::{self, SoftInterrupt, SoftInterruptFunction };

mod helpers;

#[path = "../sdk/src/lib/int_surface_sdl2.rs"]
mod int_surface_sdl2;

const W: usize = 64;
const H: usize = 40;
const SCALE: f32 = 5.0;

fn main() -> Result<(), String> {        
    let tables: Vec<(&str, &[SoftInterruptFunction])> = [
        (interrupts::proc::NAME, interrupts::proc::FUNCTIONS.as_slice()),
        (interrupts::math::NAME, interrupts::math::FUNCTIONS.as_slice()),
        (interrupts::string::NAME, interrupts::string::FUNCTIONS.as_slice()),
        (interrupts::random::NAME, interrupts::random::FUNCTIONS.as_slice()),
        (interrupts::surface::NAME, interrupts::surface::FUNCTIONS.as_slice()),
        (interrupts::trig::NAME, interrupts::trig::FUNCTIONS.as_slice())
    ].to_vec();

    let Some(source) = helpers::load_source("examples/programs/primitives.txt", &tables) else { return Ok(()) };
    
    match VirtMach::compile(source.name.as_str(), source.code.as_str(), tables) {
        Ok(res) => {                    
            let program = res.0;

            let mut vm = VirtMach::new();
            
            vm.load_program(program);                                                          

            let sdl_context = sdl2::init()?;
            let video_subsystem = sdl_context.video()?;

            let window = video_subsystem
                .window("virtmach-rs example: surface_sdl2", (W as f32 * SCALE) as u32, (H as f32 * SCALE) as u32)
                .position_centered()
                .opengl()
                .build()
                .map_err(|e| e.to_string())?;

            let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;                                

            canvas.set_scale(SCALE, SCALE)?;

            canvas.set_draw_color(Color::RGB(0, 0, 0));
            canvas.clear();
            canvas.present();                    

            let mut event_pump = sdl_context.event_pump()?;

            print!("\x1b[2J");

            'running: loop {
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. }
                        | Event::KeyDown {
                            keycode: Some(Keycode::Escape),
                            ..
                        } => break 'running,
                        _ => {}
                    }
                }                        

                // at their INDEX, time is dummy
                let interrupts: &mut [&mut dyn SoftInterrupt] = &mut [
                    &mut interrupts::math::Interrupt {},
                    &mut interrupts::proc::Interrupt {},
                    &mut interrupts::string::Interrupt {},
                    &mut interrupts::random::Interrupt {},
                    &mut interrupts::dummy::Interrupt {},
                    &mut interrupts::trig::Interrupt {},
                    &mut int_surface_sdl2::IntSurface { canvas: &mut canvas, clip: [0, 0, W as i32, H as i32 ] }
                ];                            
                
                vm.run(4096, interrupts);

                match vm.state {
                    virtmach::Runtime::Run => {}
                    _ => { canvas.present(); }
                }
                
                let mut dashboard = String::new();
                vm.write_dashboard(&mut dashboard, 0b111, 6);
                print!("\x1b[H{}", dashboard);
                helpers::print_variables_beside(&vm, &source.variables, &dashboard, 1, 1);
                
                thread::sleep(time::Duration::from_millis(1000 / 15))
            }                    
        }
        Err(err) => println!("compile error: {:?}", err)
    }
    Ok(())
}