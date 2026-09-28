REM Sums and a factorial with FOR/NEXT loops
REM After END: SUM = 55, FACT = 5040, EVENS = 30
SUM = 0
FOR I = 1 TO 10
  SUM = SUM + I
NEXT I

FACT = 1
FOR I = 7 TO 1 STEP -1
  FACT = FACT * I             ' * is computed by math.mul
NEXT

EVENS = 0
FOR I = 0 TO 10 STEP 2
  EVENS = EVENS + I
NEXT I
END
