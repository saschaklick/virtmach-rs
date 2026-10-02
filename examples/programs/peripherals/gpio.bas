REM Running light on the outputs 0-3, the pins 4-7 are inputs with pull resistors:
REM 4 and 6 pull up, 5 pulls down, 7 is an open-drain output let go high with a pull-up.
REM write_mask sets all four outputs at once, read_mask reads the inputs back.
REM After END: IN = 208 (0xd0, pins 4, 6 and 7 high), LEVEL = 1, LOW5 = 0
REQ gpio, time
FOR P = 0 TO 3
  gpio.setup(P, 1)
NEXT P
gpio.set_pull(4, 1)
gpio.set_pull(5, 2)
gpio.set_pull(6, 1)
gpio.setup(7, 2)
gpio.set_pull(7, 1)
gpio.high(7)
FOR I = 0 TO 11
  gpio.write_mask(0, 0x0F, 1 << (I MOD 4))
  IN = gpio.read_mask(0, 0xF0)
  OK = time.wait_until(0, 250)
NEXT I
gpio.toggle(0)
LEVEL = gpio.read(4)
LOW5 = gpio.read(5)
END
