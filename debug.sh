#!/bin/bash

RUST_LOG=debug cargo run -- gs://fuse-tmp /tmp/objectfs
