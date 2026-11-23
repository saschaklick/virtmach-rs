use virtmach::{ VirtMach, VMAtom, RuntimeError, color::Color, interrupts::{ SoftInterrupt, SoftInterruptFunction, surface } };

pub struct IntSurface <'a> {    
    pub w: i32,
    pub h: i32,
    pub clip: [i32;4],
    pub bitmap: &'a mut [u8]
}

impl IntSurface <'_> {
    /// A bitmap has two colors. A color atom sets a pixel if it has a bright channel and clears it if it is black
    /// or dark. A transparent color (alpha 0) is None, it changes nothing.
    fn pen(color: VMAtom) -> Option<bool> {
        let [r, g, b, a] = Color::to_rgba(color);
        (a != 0).then(|| r.max(g).max(b) >= 128)
    }

    fn draw_pixel(&mut self, x: i32, y: i32, pen: Option<bool>) {
        let Some(on) = pen else { return };
        if x >= self.clip[0] && x < self.w && x < self.clip[2] && y >= self.clip[1] && y < self.h as i32 && y < self.clip[3] {
            let pixel = y * self.w + x;
            match on {
                false => self.bitmap[(pixel / 8) as usize] &= !(1 << (7 - (x % 8))),
                true => self.bitmap[(pixel / 8) as usize] |= 1 << (7 - (x % 8))
            }        
        }
    }
}

impl SoftInterrupt for IntSurface <'_> {
    fn name(&self) -> &str {
        return surface::NAME;
    }

    fn functions(&self) -> &'static [SoftInterruptFunction<'static>] where Self:Sized {
        return &surface::FUNCTIONS;
    }
    
    fn call(&mut self, vm: &mut VirtMach) {                
        let op = vm.stack_pop();        
        
        match op {
            0 => {
                if let Some(on) = Self::pen(vm.stack_pop()) { self.bitmap.fill(if on { 0xff } else { 0x00 }); }
            }     
            1 | 6 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();                
                match op {
                    1 => {
                        let on = Self::pen(vm.stack_pop());
                        self.draw_pixel(x as i32, y as i32, on);
                    }
                    _ => {
                        let _image = vm.stack_pop();
                        for i in 0..5 { self.draw_pixel(x as i32 + i, y as i32 + i, Some(true)); self.draw_pixel(x as i32 + 4 - i, y as i32 + i, Some(true)); }
                    }
                }
            }    
            2 | 3 | 5 => {
                let x = vm.stack_pop();
                let y = vm.stack_pop();
                let w = vm.stack_pop();
                let h = vm.stack_pop();
                // the fifth argument of a border is an index, of a rectangle a color
                let argument = vm.stack_pop();
                let on = Self::pen(argument);
                match op {
                    2 => {
                        for y in y..y + h { self.draw_pixel(x as i32, y as i32, on); self.draw_pixel((x + w - 1) as i32, y as i32, on); }
                        for x in x..x + w { self.draw_pixel(x as i32, y as i32, on); self.draw_pixel(x as i32, (y + h - 1) as i32, on); }
                    }
                    3 => for y in y..y + h { for x in x..x + w { self.draw_pixel(x as i32, y as i32, on); } },
                    5 => {                        
                        for by in y..y + h {
                            let c = if by != y && by != y + h - 1 && ((by - y) % 2 == 0 || by < y + 2 || by >= y + h - 3) { 1 } else { 0 };
                            self.draw_pixel(x as i32, by as i32, Some(c != 0));
                            self.draw_pixel((x + w - 1) as i32, by as i32, Some(c != 0));
                        }
                        for bx in x..x + w {
                            let c = if bx != x && bx != x + w - 1 && ((bx - x) % 2 == 0 || bx < x + 2 || bx >= x + w - 3) { 1 } else { 0 };
                            self.draw_pixel(bx as i32, y as i32, Some(c != 0)); self.draw_pixel(bx as i32, (y + h - 1) as i32, Some(c != 0));
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
                let on = Self::pen(vm.stack_pop());

                let dx = (x_1 - x_0).abs();
                let sx = if x_0 < x_1 { 1 } else { -1 };
                let dy = -(y_1 - y_0).abs();
                let sy = if y_0 < y_1 { 1 } else { -1 };
                let mut error = dx + dy;
                let mut x = x_0;
                let mut y = y_0;
            
                loop {
                    self.draw_pixel(x as i32, y as i32, on);            
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
            10 => { vm.stack_pop(); vm.stack_pop(); vm.stack_pop(); vm.stack_pop(); }
            15 => { vm.stack_pop(); vm.stack_pop(); vm.stack_push(0); vm.stack_push(0); }
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
            20 => {
                // a bitmap has the two colors off and on, there is no palette to change, only the index is checked
                let (index, _color) = (vm.stack_pop(), vm.stack_pop());
                if !(0..=1).contains(&index) { vm.error = RuntimeError::InterruptError; }
            }
            21 => {
                let (r, g, b) = (vm.stack_pop(), vm.stack_pop(), vm.stack_pop());
                vm.stack_push(Color::from_rgb(r, g, b));
            }
            _ => { vm.error = RuntimeError::UnimplementedInterruptFunc; }
        }        
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Calls a surface function, the arguments are popped in the order given
    fn call(op: VMAtom, args: &[VMAtom]) -> (VirtMach<'static>, RuntimeError) {
        let mut buf = [0u8; 8];
        let mut surface = IntSurface { w: 8, h: 8, clip: [0, 0, 8, 8], bitmap: &mut buf };
        let mut vm = VirtMach::new();
        // a marker below the arguments shows what the function leaves on the stack
        vm.stack_push(777);
        for arg in args.iter().rev() { vm.stack_push(*arg); }
        vm.stack_push(op);
        surface.call(&mut vm);
        let error = vm.error.clone();
        (vm, error)
    }

    /// The bitmap of 8 x 8 pixels after a drawing function, from a bitmap with all bits set to the given value
    fn draw(initial: u8, op: VMAtom, args: &[VMAtom]) -> [u8; 8] {
        let mut buf = [initial; 8];
        {
            let mut surface = IntSurface { w: 8, h: 8, clip: [0, 0, 8, 8], bitmap: &mut buf };
            let mut vm = VirtMach::new();
            for arg in args.iter().rev() { vm.stack_push(*arg); }
            vm.stack_push(op);
            surface.call(&mut vm);
            assert_eq!(vm.error, RuntimeError::NoError);
        }
        buf
    }

    #[test]
    fn colors_decide_if_a_pixel_is_on() {
        // white and the bright palette colors draw, black and dark colors erase, a transparent color does nothing
        assert_eq!(draw(0, 1, &[0, 0, Color::vga(15)])[0], 0x80);
        assert_eq!(draw(0, 1, &[1, 0, Color::vga(12)])[0], 0x40);
        assert_eq!(draw(0xff, 1, &[0, 0, Color::vga(0)])[0], 0x7f);
        assert_eq!(draw(0xff, 1, &[0, 0, Color::vga(8)])[0], 0x7f);
        assert_eq!(draw(0, 1, &[0, 0, Color::from_rgba(255, 255, 255, 0)])[0], 0x00);
        assert_eq!(draw(0xff, 1, &[0, 0, Color::from_rgba(0, 0, 0, 0)])[0], 0xff);
        // clear
        assert_eq!(draw(0, 0, &[Color::vga(15)]), [0xff; 8]);
        assert_eq!(draw(0xff, 0, &[Color::vga(0)]), [0; 8]);
        // lines and filled rectangles
        assert_eq!(draw(0, 4, &[0, 3, 7, 3, Color::vga(15)])[3], 0xff);
        assert_eq!(draw(0, 3, &[0, 0, 2, 2, Color::vga(15)])[..3], [0xc0, 0xc0, 0x00]);
    }

    #[test]
    fn rgb_returns_the_color_atom() {
        let (mut vm, error) = call(21, &[100, 0, 50]);
        assert_eq!(error, RuntimeError::NoError);
        assert_eq!(vm.stack_pop(), Color::from_rgba(100, 0, 50, 255));
        assert_eq!(vm.stack_pop(), 777, "the three arguments are replaced by the color");
        // channels outside of 0..255 are cut
        let (mut vm, _) = call(21, &[VMAtom::MAX, VMAtom::MIN, 0]);
        assert_eq!(vm.stack_pop(), Color::from_rgb(VMAtom::MAX, 0, 0));
    }

    #[test]
    fn set_color_takes_the_two_colors_of_a_bitmap() {
        for index in [0, 1] {
            let color = Color::from_rgb(1, 2, 3);
            let (mut vm, error) = call(20, &[index, color]);
            assert_eq!(error, RuntimeError::NoError);
            assert_eq!(vm.stack_pop(), 777, "both arguments are popped and nothing is returned");
        }
        assert_eq!(call(20, &[2, 0]).1, RuntimeError::InterruptError);
        assert_eq!(call(20, &[-1, 0]).1, RuntimeError::InterruptError);
    }
}
