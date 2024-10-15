#!/bin/bash

mkdir -p target
gcc -Wall -DDEBUG objectfs.c -lssl -lcrypto `pkg-config fuse3 --cflags --libs` -o target/objectfs