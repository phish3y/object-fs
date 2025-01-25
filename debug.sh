#!/bin/bash

RUST_LOG=debug cargo run -- s3://fuse-tmp /tmp/objectfs
