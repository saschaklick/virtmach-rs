#!/bin/sh
# Tests the firmware in simavr: builds it with a test program from tests/ for every case, runs it with
# sim/simtest.c, which plays the outside world (UART RX bytes, an I2C device) and decodes what the
# firmware sends (UART TX bytes, math results as nibbles), and compares that with tests/NAME.expect.
#   ./test.sh [case ...]    all cases if none is given, see the list at the bottom
# Needs simavr with its headers (SIMAVR_PREFIX, /usr/local by default) and gcc for simtest.
set -e
cd "$(dirname "$0")"
OUT=target/tests
SIMAVR_PREFIX="${SIMAVR_PREFIX:-/usr/local}"
mkdir -p "$OUT"
ROOT="$(cd ../.. && pwd)"

# the BASIC compiler is built from the repository root, which has no toolchain file for the avr
(cd "$ROOT" && cargo build -q --manifest-path sdk/Cargo.toml --bin basic)
BASIC="$ROOT/target/debug/basic"

if [ ! -x "$OUT/simtest" ] || [ sim/simtest.c -nt "$OUT/simtest" ]; then
    gcc -O2 -Wall -I"$SIMAVR_PREFIX/include/simavr" sim/simtest.c -o "$OUT/simtest" \
        -L"$SIMAVR_PREFIX/lib" -Wl,-rpath,"$SIMAVR_PREFIX/lib" -lsimavr -lm
fi

failed=0
passed=0

# build NAME PROGRAM FEATURES [SED_SCRIPT]: the firmware for tests/PROGRAM.bas as $OUT/NAME.elf
build() {
    sed "${4:-}" "tests/$2.bas" > "$OUT/$1.bas" 2>/dev/null || cp "tests/$2.bas" "$OUT/$1.bas"
    "$BASIC" "$OUT/$1.bas" math time periph -o "$OUT/$1.bin" > "$OUT/$1.log" 2>&1 \
        || { cat "$OUT/$1.log"; return 1; }
    VIRTMACH_PROGRAM="$PWD/$OUT/$1.bin" cargo build -q --release --no-default-features --features "rt,simavr,$3" 2> "$OUT/$1.log" \
        || { cat "$OUT/$1.log"; return 1; }
    cp target/avr-none/release/virtmach-attiny85.elf "$OUT/$1.elf"
}

# run NAME MILLISECONDS ARGS...: what simtest finds as $OUT/NAME.out
run() {
    name=$1; ms=$2; shift 2
    "$OUT/simtest" "$OUT/$name.elf" "$ms" "$@" 2>/dev/null | grep -E '^(tx|txerror|value|i2c)' > "$OUT/$name.out" || true
}

# the lines of $OUT/NAME.out and tests/EXPECT.expect have to be the same
check_exact() {
    diff -u "tests/$2.expect" "$OUT/$1.out" > "$OUT/$1.diff"
}

# the values of $OUT/NAME.out have to be the ones in tests/EXPECT.expect, which has a tolerance for each
check_values() {
    awk -v out="$OUT/$1.out" '
        BEGIN {
            while ((getline line < out) > 0) {
                split(line, f, " ")
                if (f[1] == "value") { n++; v[n] = f[2]; first[n] = f[3]; last[n] = f[4] }
            }
        }
        /^#/ || NF == 0 { next }
        $1 == "gap" { gap = $2; next }
        {
            i++
            label = $0; sub(/^[a-z]+ [^ ]+ [^ ]+ /, "", label)
            if (i > n) { printf "missing value %d: %s\n", i, $0; bad++; next }
            ok = 1
            if ($1 == "range") {
                label = $0; sub(/^range [^ ]+ [^ ]+ /, "", label)
                ok = (v[i] >= $2 && v[i] <= $3); want = $2 ".." $3
            } else {
                d = v[i] - $2
                if ($1 == "valuemod") d = ((d + 32768) % 65536 + 65536) % 65536 - 32768
                if (d < 0) d = -d
                ok = (d <= $3 + 1e-9); want = $2 " +-" $3
            }
            if (gap != "" && first[i] - last[i - 1] < gap) { ok = 0; want = want " after at least " gap " ms, was " (first[i] - last[i - 1]) }
            gap = ""
            if (!ok) { printf "wrong %s: got %s, wanted %s\n", label, v[i], want; bad++ }
        }
        END {
            if (n != i) { printf "%d values, wanted %d\n", n, i; bad++ }
            exit bad ? 1 : 0
        }' "tests/$2.expect" > "$OUT/$1.diff"
}

# case NAME CHECK EXPECT PROGRAM FEATURES SED_SCRIPT MILLISECONDS ARGS...
run_case() {
    name=$1; check=$2; expect=$3; program=$4; features=$5; script=$6; ms=$7; shift 7
    if [ -n "$SELECTED" ] && ! echo " $SELECTED " | grep -q " $name "; then return; fi
    printf '%-18s' "$name"
    if ! build "$name" "$program" "$features" "$script"; then echo "BUILD FAILED"; failed=$((failed + 1)); return; fi
    run "$name" "$ms" "$@"
    if "$check" "$name" "$expect"; then
        echo ok; passed=$((passed + 1))
    else
        echo FAILED; cat "$OUT/$name.diff"; failed=$((failed + 1))
    fi
}

SELECTED="$*"

# UART TX in every config, the bytes are decoded at the baud rate and config of uart.setup
tx_case() { # NAME BAUD/10 CONFIG
    run_case "uart_tx_$1" check_exact "uart_tx_$1" uart_tx rt,uart_tx "s/@BAUD@/$2/;s/@CONFIG@/$3/" 300 --tx "$(( $2 * 10 ))" "$3"
}
tx_case 8n1 240 0
tx_case 8n2 120 16
tx_case 8e1 480 4
tx_case 7o1 960 9
tx_case 5n1 240 3

# UART RX, bytes are sent to the firmware and come back plus one
rx_case() { # NAME BAUD/10 CONFIG
    run_case "uart_rx_$1" check_exact "uart_rx_$1" uart_rx rt,uart_tx,uart_rx "s/@BAUD@/$2/;s/@CONFIG@/$3/" 400 \
        --tx "$(( $2 * 10 ))" "$3" --rx "$(( $2 * 10 ))" "$3" 50 0x41 0x30 0x55
}
rx_case 8n1 240 0
rx_case 8e1 480 4
rx_case 7o1 240 9
rx_case 8n2 120 16
run_case uart_mem check_exact uart_mem uart_mem rt,uart_tx,uart_rx "" 400 --tx 2400 0 --rx 2400 0 50 10 20 30

# I2C against a device at 0x50, at the wrong address, and with a line held low
run_case i2c check_exact i2c i2c rt,uart_tx,i2c "" 400 --tx 9600 0 --i2c 0x50
run_case i2c_nack check_exact i2c_nack i2c_error rt,uart_tx,i2c "" 200 --tx 9600 0 --i2c 0x51
run_case i2c_hold_sda check_exact i2c_hold_sda i2c_error rt,uart_tx,i2c "" 200 --tx 9600 0 --hold-sda
run_case i2c_hold_scl check_exact i2c_hold_scl i2c_error rt,uart_tx,i2c "" 300 --tx 9600 0 --hold-scl

# math, split in two firmwares to fit the flash
run_case math_trig check_values math_trig math_trig rt,gpio,time,math_trig "" 3000 --nibbles
run_case math_other check_values math_other math_other rt,gpio,time,math_random,math_sqr,math_pow "" 3000 --nibbles

echo "$passed passed, $failed failed"
[ "$failed" -eq 0 ]
