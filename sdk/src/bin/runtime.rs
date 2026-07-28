use std::{thread, time, fs::File, io::Read };
use simple_logger;
use clap::Parser;
use bytes::{ BytesMut };
use virtmach::{ VirtMach, Program, interrupts::{ SoftInterrupt, Proc, Math, Random } };
use bitmap_writer::{Bitmap, Writer, Frame, Style};

#[derive(Parser, Debug)]
#[command(name = "virtmach-rs Compiler")]
#[command(version = "0.1")]
#[command(about = "Compile virtmach-rs listings into binary", long_about = None)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(help = "Binary file to run")]
    bin: String,    
    
    #[arg(short, long, help = "Choose \"term\" or \"sdl2\" for the surface interrupt functions")]
    surface: Option<String>,

    #[arg(short, long, default_value_t = 1, help = "Verbosity level")]
    verbose: u32    
}

fn main() -> Result<(), String> {        
    let args = Args::parse();
    
    let verbose = args.verbose;
    match verbose {
        0 => {}
        _ => simple_logger::init_with_level(match args.verbose {
            1 => log::Level::Error,
            2 => log::Level::Info,
            3 => log::Level::Debug,
            _ => log::Level::Trace
        }).unwrap()
    }

    let mut surface_buf = BytesMut::with_capacity(64 * 40 / 8);

    let surface_str = args.surface.unwrap_or(String::new());
    let surface = surface_str.as_str();

    let filename = args.bin.as_str();    
    
    match File::open(filename) {
        Ok(mut file) => {
            let mut bin = vec![];
            match file.read_to_end(&mut bin) {
                Ok(bin_len) => {
                    println!("loaded binary {} ({}b)", filename, bin_len);
                    
                    let program = Program { source: 0, id: &filename, data: &bin[..] };

                    let mut vm = VirtMach::new();          

                    vm.load_program(program);

                    let mut buf = &mut [0u8;1024];

                    let interrupts: &mut [&mut dyn SoftInterrupt] = match surface {
                        "term" => &mut [ &mut Proc {}, &mut Math {}, &mut Random {}, &mut int_surface_term::IntSurface { w: 64, h: 40, clip: [0, 0, 63, 39], bitmap: buf } ],
                        _ => &mut [ &mut Proc {}, &mut Math {}, &mut Random {}]
                    };                                             

                    loop {
                        vm.run(1, interrupts);
                        
                        if verbose >= 1 {
                            let mut dashboard = String::new();
                            vm.write_dashboard(&mut dashboard, 0b111, 5);

                            match surface {
                                "term" => {
                                    let bitmap = Bitmap::new(64, 40, buf);
                                    print!("\x1b[J");                                            
                                    let mut w = Writer::new();
                                    w.frame(Frame::UnicodeDoubleUFrame)
                                    .style(Style::UnicodeBlock1x2)
                                    .ansi_position(1, 1);
                                    w.print(&bitmap);
                                    if verbose >= 2 { for (i, line) in dashboard.lines().enumerate() { print!("\x1b[{};{}H {}\x1b[K", i + 1, 64 + 3, line); } }
                                    println!("");
                                }
                                _ => {
                                    if verbose >= 2 {
                                        print!("\x1b[H\x1b[J");          
                                        println!("{}", dashboard);
                                    }
                                }
                            }                            
                        }

                        thread::sleep(time::Duration::from_millis(1000 / 60));

                        if false { break; }
                    }  

                    Ok(())            
                }
                Err(err) => { Err(format!("failed to read binary file: {}", err)) }
            }
        }
        Err(err) => { Err(format!("failed to open binary: {}", err)) }
    }    
}


#[path = "../lib/int_surface_term.rs"]
mod int_surface_term;

fn surface_term_init() {

}