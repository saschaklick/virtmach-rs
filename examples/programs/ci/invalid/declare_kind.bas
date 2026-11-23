REM expect: declared as a SUB but defined as a FUNCTION
DECLARE SUB F
FUNCTION F
  F = 1
END FUNCTION
