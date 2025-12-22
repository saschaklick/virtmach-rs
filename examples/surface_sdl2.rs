#![allow(static_mut_refs)]

extern crate sdl2;
use sdl2::{ event::Event, keyboard::Keycode, pixels::Color };

use std::{thread, time};
use virtmach::VirtMach;
use virtmach::interrupts::{self, SoftInterrupt, Proc, Math, Random };

mod helpers;

mod int_surface_sdl2;

const W: usize = 64;
const H: usize = 40;
const SCALE: f32 = 5.0;

fn main() -> Result<(), String> {
    match helpers::load_file("examples/programs/primitives.txt") {
        Ok(content) => {            
            match VirtMach::compile(content.0.as_str(), content.1.as_str(), [(String::from(interrupts::SurfaceMap.0), String::from(interrupts::SurfaceMap.1))].to_vec()) {
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

                        let interrupts: &mut [&mut dyn SoftInterrupt] = &mut [ &mut Proc {}, &mut Math {}, &mut Random {}, &mut int_surface_sdl2::IntSurface { canvas: &mut canvas, clip: [0, 0, W as i32, H as i32 ] }];                            
                        
                        vm.run(1024, interrupts);

                        canvas.present();        

                        let mut dashboard = String::new();
                        vm.write_dashboard(&mut dashboard, 0b111, 6);
                        print!("\x1b[H{}", dashboard);
                        
                        thread::sleep(time::Duration::from_millis(1000 / 15))
                    }                    
                }
                Err(err) => println!("compile error: {:?}", err)
            }            
        } 
        Err(err) => println!("file read error: {:?}", err)               
    }
    Ok(())
}