REM Rolls two dice 100 times using the random interrupt
REM After END: DOUBLES = number of doubles, LO/HI = lowest/highest sum
REQ random
DOUBLES = 0 : LO = 12 : HI = 2
FOR N = 1 TO 100
  A = random.range(1, 6)
  B = random.range(1, 6)
  IF A = B THEN DOUBLES = DOUBLES + 1
  S = A + B
  IF S < LO THEN LO = S
  IF S > HI THEN HI = S
NEXT N
END
