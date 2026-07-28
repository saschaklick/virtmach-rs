use virtmach::{ VirtMach, interrupts };

mod helpers;

fn main(){
    match helpers::load_file("examples/programs/count.txt") {       
        Ok(content) => {
            match VirtMach::compile(content.0.as_str(), content.1.as_str(), [
                (interrupts::proc::NAME, interrupts::proc::FUNCTIONS.as_slice()),
                (interrupts::math::NAME, interrupts::math::FUNCTIONS.as_slice()),
                (interrupts::random::NAME, interrupts::random::FUNCTIONS.as_slice()),                
            ].to_vec()) {
                Ok(res) => {                    
                    let program = res.0;
                    helpers::disassemble(program);                                                         
                }
                Err(err) => println!("compile error: {:?}", err)
            }
        }
        Err(err) =>  println!("file read error: {:?}", err)                                                    
    }
}