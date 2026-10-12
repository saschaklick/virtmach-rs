REM UART with memory: sends 4 cells, waits for 3 received bytes, reads them into memory and sends
REM them back with the counts
REQ periph
DIM A(3)
periph.uart_setup(0, 240, 0)
A(0) = 1 : A(1) = 2 : A(2) = 3 : A(3) = 4
periph.uart_write(0, periph.uart_write_mem(0, 0, 4))
WHILE periph.uart_available(0) < 3
WEND
periph.uart_write(0, periph.uart_read_mem(0, 0, 3))
periph.uart_write(0, periph.uart_write_mem(0, 0, 3))
END
