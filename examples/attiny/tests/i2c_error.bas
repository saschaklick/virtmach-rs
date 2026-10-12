REM I2C probe of the device 0x50 whose result, the status, is sent as two bytes on the UART TX
REQ math, periph
periph.uart_setup(0, 960, 0)
periph.i2c_setup(0, 100)
X = periph.i2c_probe(0, 80)
periph.uart_write(0, (X >> 8) AND 255)
periph.uart_write(0, X AND 255)
END
