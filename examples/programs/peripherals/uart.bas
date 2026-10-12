REM Sends bytes and a text over uart 0, the simulated uart loops them back, so the
REM program receives them again and reads them back one by one.
REM After END: SENT = 3, WAITING = 7, SUM = 370 (16+32+48+64 and "Hi!"), NONE = -1
REQ periph, time
periph.uart_setup(0, 11520, 0)
FOR I = 1 TO 4
  periph.uart_write(0, I * 16)
  OK = time.wait_until(0, 250)
NEXT I
SENT = periph.uart_write_str(0, "Hi!", 0, 3)
WAITING = periph.uart_available(0)
SUM = 0
WHILE periph.uart_available(0) > 0
  B = periph.uart_read(0)
  SUM = SUM + B
  OK = time.wait_until(0, 250)
WEND
NONE = periph.uart_read(0)
END
