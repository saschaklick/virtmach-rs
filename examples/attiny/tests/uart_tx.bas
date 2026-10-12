REM UART TX test, the bytes are decoded from PB2 by simtest. @BAUD@ and @CONFIG@ are filled in by test.sh.
REQ periph
periph.uart_setup(0, @BAUD@, @CONFIG@)
periph.uart_write(0, 72)
periph.uart_write(0, 105)
periph.uart_write(0, 33)
periph.uart_write(0, 0)
periph.uart_write(0, 255)
periph.uart_write(0, 85)
periph.uart_write(0, 170)
END
