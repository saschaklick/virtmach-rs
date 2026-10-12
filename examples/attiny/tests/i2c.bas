REM I2C test: a device at 0x50 with 8 registers is played by simtest, the results of every call
REM leave as two bytes, high and low, on the UART TX at 9600 baud.
REQ math, periph
DEV = 80
DIM A(3)
periph.uart_setup(0, 960, 0)
periph.i2c_setup(0, 100)

SUB SEND(V)
  periph.uart_write(0, (V >> 8) AND 255)
  periph.uart_write(0, V AND 255)
END SUB

SEND periph.i2c_probe(0, DEV)
SEND periph.i2c_probe(0, 81)
SEND periph.i2c_write_reg(0, DEV, 2, 90)
SEND periph.i2c_read_reg(0, DEV, 2)
SEND periph.i2c_write_byte(0, DEV, 6)
SEND periph.i2c_read_byte(0, DEV)
A(0) = 7 : A(1) = 8 : A(2) = 9 : A(3) = 10
SEND periph.i2c_write_mem(0, DEV, 0, 4)
SEND periph.i2c_write_byte(0, DEV, 7)
SEND periph.i2c_read_mem(0, DEV, 0, 2)
SEND A(0) * 256 + A(1)
SEND periph.i2c_write_read_mem(0, DEV, 0, 1, 2, 3)
SEND A(2) * 256 + A(3)
SEND periph.i2c_write_byte(0, 0, 1)
SEND periph.i2c_read_reg(0, 200, 1)
END
