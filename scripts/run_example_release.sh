#!/bin/sh

# Run an example with the specified arguments
cargo build --release --example $1 && sudo ./target/release/examples/$1 $2
