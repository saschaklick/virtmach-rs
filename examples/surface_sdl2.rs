#![allow(static_mut_refs)]

extern crate sdl2;
use sdl2::{ event::Event, keyboard::Keycode, pixels::Color };

use std::{thread, time};
use virtmach::VirtMach;
use virtmach::interrupt_set;

mod helpers;

#[path = "../sdk/src/lib/int_surface_sdl2.rs"]
mod int_surface_sdl2;

const W: usize = 64;
const H: usize = 40;
const SCALE: f32 = 5.0;

// The interrupts once, for the tables to compile with and for the vm to run with. The `_` ones
// are the variables of main, given where the list is made.
interrupt_set!(interrupts = [math, proc, string, time => _, periph => _, surface => _]);

fn main() -> Result<(), String> {
    let tables = interrupts!(tables);

    let Some(source) = helpers::load_source("examples/programs/gfx/primitives.txt", &tables) else { return Ok(()) };

    match VirtMach::compile_tables(source.name.as_str(), source.code.as_str(), &tables) {
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

            let mut time = helpers::SimTime::default();
            let mut hw = helpers::SimPeripherals::default();

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

                let ready = time.ready();
                // it borrows the canvas only while the vm runs, then the canvas is presented
                let mut surface = int_surface_sdl2::IntSurface { canvas: &mut canvas, clip: [0, 0, W as i32, H as i32 ] };
                interrupts!(let list, &mut time, &mut hw, &mut surface);

                if ready { vm.run(4096, list); time.after_run(&vm); }

                // not while a wait keeps the vm paused, nothing new was drawn
                match vm.state {
                    virtmach::Runtime::Run => {}
                    _ if ready => { canvas.present(); }
                    _ => {}
                }

                let mut dashboard = String::new();
                vm.write_dashboard(&mut dashboard, 0b111, 6);
                print!("\x1b[H{}", dashboard);
                helpers::print_variables_beside(&vm, &source.variables, &dashboard, 1, 1);
                helpers::print_hardware_report(&hw, 1, helpers::report_column(&dashboard, &source.variables, 1));

                thread::sleep(time::Duration::from_millis(1000 / 15))
            }
        }
        Err(err) => println!("compile error: {:?}", err)
    }
    Ok(())
}
