#!/bin/bash

mkdir -p target
gcc -Wall fs.c `pkg-config fuse3 --cflags --libs` -o target/e_fs

./target/e_fs -s -f /tmp/fuse
