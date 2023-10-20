#!/bin/sh

# Run an example with the specified arguments
cargo build --profile callgrind --example $1 && sudo valgrind --tool=callgrind --dump-instr=yes --collect-jumps=yes --simulate-cache=yes ./target/callgrind/examples/$1 $2
