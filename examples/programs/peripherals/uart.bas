REM Sends bytes and a text over uart 0, the simulated uart loops them back, so the
REM program receives them again and reads them back one by one.
REM After END: SENT = 3, WAITING = 7, SUM = 370 (16+32+48+64 and "Hi!"), NONE = -1
REQ uart, time
uart.setup(0, 11520, 0)
FOR I = 1 TO 4
  uart.write(0, I * 16)
  OK = time.wait_until(0, 250)
NEXT I
SENT = uart.write_str(0, "Hi!", 0, 3)
WAITING = uart.available(0)
SUM = 0
WHILE uart.available(0) > 0
  B = uart.read(0)
  SUM = SUM + B
  OK = time.wait_until(0, 250)
WEND
NONE = uart.read(0)
END
