REM PRINT, ?, CLS and COLOR on the surface interrupt (build with --cfg basic_console="surface"), LOCATE is dropped.
REM Every item is drawn at the text cursor, numbers get a blank in front of and behind them, `,` goes to the
REM next zone of 14 characters and a PRINT that does not end in ; or , starts a new line.
DEFINT A-Z
REQ surface, math, string
CLS
PRINT "Hello"
PRINT "a"; "b"
PRINT 42
PRINT 1, 2
PRINT "x";
PRINT "y"
LOCATE 5, 5
? "q"
PRINT
IF 1 THEN PRINT "end" ELSE PRINT "never"
END
