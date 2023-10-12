#!/bin/sh

# Run an example with the specified arguments
cargo build --example $1 && sudo RUST_BACKTRACE=1 ./target/debug/examples/$1 $2
