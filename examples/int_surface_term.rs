use virtmach::{ VirtMach, VMAtom, RuntimeError, interrupts::{ SoftInterrupt, SoftInterruptDef, SoftInterruptFunction } };

pub static DEF: SoftInterruptDef = SoftInterruptDef {
    name: "surface",
    functions: &[        
        SoftInterruptFunction { no:  0, name: "clear", arguments: 1, returns: 0 },            
        SoftInterruptFunction { no:  1, name: "draw_pixel", arguments: 3, returns: 0 },       
        SoftInterruptFunction { no:  2, name: "draw_rect", arguments: 5, returns: 0 },      
        SoftInterruptFunction { no:  3, name: "fill_rect", arguments: 5, returns: 0 },      
        SoftInterruptFunction { no:  4, name: "draw_line", arguments: 5, returns: 0 },      
        SoftInterruptFunction { no:  5, name: "draw_border", arguments: 5, returns: 0 },      
        SoftInterruptFunction { no:  6, name: "draw_image", arguments: 3, returns: 0 },             
        SoftInterruptFunction { no: 16, name: "get_size", arguments: 0, returns: 2 },                    
        SoftInterruptFunction { no: 17, name: "get_image_size", arguments: 1, returns: 2 },                     
        SoftInterruptFunction { no: 18, name: "get_clip", arguments: 0, returns: 4 },            
        SoftInterruptFunction { no: 19, name: "set_clip", arguments: 4, returns: 0 },        
    ]
};

pub struct IntSurface <'a> {    
    pub w: i32,
    pub h: i32,
    pub clip: [i32;4],
    pub bitmap: &'a mut [u8]
}

impl IntSurface <'_> {
    fn draw_pixel(&mut self, x: i32, y: i32, color: u8) {
        if x >= self.clip[0] && x < self.w && x < self.clip[2] && y >= self.clip[1] && y < self.h as i32 && y < self.clip[3] {
            let pixel = y * self.w + x;
            match color {
                0 => self.bitmap[(pixel / 8) as usize] &= !(1 << (7 - (x % 8))),
                _ => self.bitmap[(pixel / 8) as usize] |= 1 << (7 - (x % 8))
            }        
        }
    }
}

impl SoftInterrupt for IntSurface <'_> {
    fn name(&self) -> &str {
        return "surface";
    }
    
    fn call(&mut self, vm: &mut VirtMach) {                
        let op = vm.stack_pop();        
        
        match op {
            0 => {
                let color = vm.stack_pop() as u8;  
                self.bitmap.fill(if color == 0 { 0x00 } else { 0xff });
            }     
            1 | 6 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();                
                match op {
                    1 => {
                        let color = vm.stack_pop();                
                        self.draw_pixel(x as i32, y as i32, color as u8);
                    }
                    _ => {
                        let _image = vm.stack_pop();
                        for i in 0..5 { self.draw_pixel(x as i32 + i, y as i32 + i, 1 as u8); self.draw_pixel(x as i32 + 4 - i, y as i32 + i, 1 as u8); }
                    }
                }
            }    
            2 | 3 | 5 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();
                let w = vm.stack_pop();
                let h = vm.stack_pop();
                let color = vm.stack_pop();                                         
                match op {
                    2 => {
                        for y in y..y + h { self.draw_pixel(x as i32, y as i32, color as u8); self.draw_pixel((x + w - 1) as i32, y as i32, color as u8); }
                        for x in x..x + w { self.draw_pixel(x as i32, y as i32, color as u8); self.draw_pixel(x as i32, (y + h - 1) as i32, color as u8); }
                    }
                    3 => for y in y..y + h { for x in x..x + w { self.draw_pixel(x as i32, y as i32, color as u8); } },
                    5 => {                        
                        for by in y..y + h {
                            let c = if by != y && by != y + h - 1 && ((by - y) % 2 == 0 || by < y + 2 || by >= y + h - 3) { 1 } else { 0 };
                            self.draw_pixel(x as i32, by as i32, c as u8);
                            self.draw_pixel((x + w - 1) as i32, by as i32, c as u8);
                        }
                        for bx in x..x + w {
                            let c = if bx != x && bx != x + w - 1 && ((bx - x) % 2 == 0 || bx < x + 2 || bx >= x + w - 3) { 1 } else { 0 };
                            self.draw_pixel(bx as i32, y as i32, c as u8); self.draw_pixel(bx as i32, (y + h - 1) as i32, c as u8);
                        }
                    }
                    _ => {}
                }
            }
            4 => {
                let x_0 = vm.stack_pop();
                let y_0 = vm.stack_pop();
                let x_1 = vm.stack_pop();
                let y_1 = vm.stack_pop();
                let color = vm.stack_pop();

                let dx = (x_1 - x_0).abs();
                let sx = if x_0 < x_1 { 1 } else { -1 };
                let dy = -(y_1 - y_0).abs();
                let sy = if y_0 < y_1 { 1 } else { -1 };
                let mut error = dx + dy;
                let mut x = x_0;
                let mut y = y_0;
            
                loop {
                    self.draw_pixel(x as i32, y as i32, color as u8);            
                    let e2 = 2 * error;
                    if e2 >= dy {
                        if x == x_1 { break; }
                        error = error + dy;
                        x = x + sx;
                    }
                    if e2 <= dx {
                        if y == y_1 { break; }
                        error = error + dx;
                        y = y + sy;
                    }
                }
            }    
            16 => {
                [self.w, self.h].iter().for_each(|v| { vm.stack_push(*v as VMAtom); });                
            }
            17 => {
                let _image = vm.stack_pop();
                [5, 5].iter().for_each(|v| { vm.stack_push(*v as VMAtom); });                
            }      
            18 => {
                self.clip.iter().for_each(|v| vm.stack_push(*v as VMAtom));
            }      
            19 => {
                self.clip = [0i32;4].map(|_| { vm.stack_pop() as i32 });
            }                                   
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }        
    }
}
