use virtmach::{ VirtMach, interrupts::{ self, SoftInterruptFunction } };

mod helpers;

fn main(){
    let tables: Vec<(&str, &[SoftInterruptFunction])> = [
        (interrupts::proc::NAME, interrupts::proc::FUNCTIONS.as_slice()),
        (interrupts::math::NAME, interrupts::math::FUNCTIONS.as_slice()),        
        (interrupts::random::NAME, interrupts::random::FUNCTIONS.as_slice()),
        (interrupts::surface::NAME, interrupts::surface::FUNCTIONS.as_slice())
    ].to_vec();

    let Some(source) = helpers::load_source("examples/programs/count.txt", &tables) else { return };

    match VirtMach::compile(source.name.as_str(), source.code.as_str(), tables) {
        Ok(res) => {
            let program = res.0;
            if !source.variables.is_empty() {
                println!();
                println!("Variables:");
                for line in helpers::variable_locations(&source.variables) { println!("{}", line); }
            }
            helpers::disassemble(program);
        }
        Err(err) => println!("compile error: {:?}", err)
    }
}
