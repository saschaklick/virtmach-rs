REM Strings: every text in double quotes becomes a #db entry of the listing,
REM which interrupt functions such as surface.draw_text take. Identical texts share one entry.
REM Write "" for a quote inside a text.
REM After END: LEN1 = 13, LEN2 = 12, LEN3 = 0, the entries are "Hello, World!", "Hi; ""quoted""", "", "same entry"
DEFINT A-Z
REQ surface, string
HELLO$ = "Hello, World!"
SAME$ = "Hello, World!"
OTHER$ = "Hi; ""quoted"""
LEN1 = LEN(HELLO$)
LEN2 = LEN(OTHER$)
LEN3 = LEN("")
surface.draw_text(1, 1, 0, HELLO$)
IF HELLO$ = SAME$ THEN surface.draw_text(1, 10, 0, "same entry")
END
