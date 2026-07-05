#!/bin/bash

set -eu

export CARGO_INCREMENTAL=0
cargo +nightly -Zscript $REPRODUCER 2>&1 | grep 'panicked at /rustc-dev/c397dae808f70caebab1fc4e11b3edf7e59f58c7/compiler/rustc_middle/src/ty/instance.rs:616:21'
