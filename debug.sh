#!/bin/bash

RUST_LOG=debug cargo run -- s3://fuse-tmp1 /tmp/objectfs
