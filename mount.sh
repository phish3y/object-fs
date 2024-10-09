#!/bin/bash

gcc -Wall fs.c `pkg-config fuse3 --cflags --libs` -o target/e_fs

./target/e_fs -s -f /tmp/fuse
# ./target/e_fs -d -s -f /tmp/fuse

#fusermount -u /tmp/fuse
