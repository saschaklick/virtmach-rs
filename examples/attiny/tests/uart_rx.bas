REM UART RX test: reads without a received byte (-1), then sends every received byte plus one back
REQ periph
periph.uart_setup(0, @BAUD@, @CONFIG@)
periph.uart_write(0, periph.uart_read(0))
WHILE 1
  IF periph.uart_available(0) > 0 THEN
    periph.uart_write(0, periph.uart_read(0) + 1)
  END IF
WEND
