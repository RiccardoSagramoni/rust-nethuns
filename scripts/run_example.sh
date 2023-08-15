#!/bin/sh

# Run an example with the specified arguments
cargo build --example $1 && sudo ./target/debug/examples/$1 $2
