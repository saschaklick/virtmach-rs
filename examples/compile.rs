use virtmach::VirtMach;

mod helpers;

fn main(){
    match helpers::load_file("examples/programs/count.txt") {       
        Ok(content) => {
            match VirtMach::compile(content.0.as_str(), content.1.as_str(), vec![]) {
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