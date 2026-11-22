use virtmach::{ VirtMach, interrupt_tables };

mod helpers;

fn main(){
    let tables = interrupt_tables!([proc, math, surface]);

    let Some(source) = helpers::load_source("examples/programs/ci/count.txt", &tables) else { return };

    match VirtMach::compile_tables(source.name.as_str(), source.code.as_str(), &tables) {
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
