REM Talks to the simulated i2c device at &H50 with 8 registers. The first byte of a write
REM sets the register pointer, reads continue from it and wrap around after register 7.
REM After END: MISSING = -1, FOUND = 0, B6 = 102 (0x66), B7 = 119 (0x77), B0 = 0,
REM            R3 = 79 ("O"), BAD = -2
DEFINT A-Z
REQ periph, time
DEV = &H50
periph.i2c_setup(0, 100)
MISSING = periph.i2c_probe(0, &H51)
FOUND = periph.i2c_probe(0, DEV)
FOR R = 0 TO 7
  N = periph.i2c_write_reg(0, DEV, R, R * &H11)
  OK = time.wait_until(0, 200)
NEXT R
N = periph.i2c_write_byte(0, DEV, 6)
B6 = periph.i2c_read_byte(0, DEV)
B7 = periph.i2c_read_byte(0, DEV)
B0 = periph.i2c_read_byte(0, DEV)
REM "3" is 0x33, register 3 of 8, the "O" and "K" go to the registers 3 and 4
N = periph.i2c_write_str(0, DEV, "3OK", 0, 3)
R3 = periph.i2c_read_reg(0, DEV, 3)
BAD = periph.i2c_read_reg(0, DEV, 8)
END
