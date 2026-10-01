#!/bin/sh
# Runs the firmware in simavr and prints when PB1 changes, with the time since the last change.
# The simavr feature adds the .mmcu section that makes simavr trace PB1 into target/blink.vcd,
# which can also be opened in GTKWave.
#   ./simulate.sh [wall clock seconds to simulate, default 2] [changes to print, default 10]
set -e
cd "$(dirname "$0")"
cargo build --release --features simavr
cd target
rm -f blink.vcd
simavr avr-none/release/virtmach-attiny85.elf > simavr.log 2>&1 &
pid=$!
sleep "${1:-2}"
kill -INT $pid
wait $pid 2>/dev/null || true
awk -v max="${2:-10}" '
    /^\$timescale/ { n = $2 + 0; unit = $2; sub(/^[0-9]+/, "", unit); scale = n * (unit == "ns" ? 1e-9 : unit == "us" ? 1e-6 : unit == "ps" ? 1e-12 : 1e-3) }
    /^#/ { t = substr($0, 2) * scale }
    /^[01]!/ && count < max { printf "%11.6f s  PB1 %s  %+.6f s\n", t, substr($0, 1, 1), t - last; last = t; count++ }
' blink.vcd
