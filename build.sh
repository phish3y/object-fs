#!/bin/bash

mkdir -p target
gcc -Wall -DDEBUG objectfs.c -laws -l awsv4 -lcrypto -lssl `pkg-config fuse3 --cflags --libs` -o target/objectfs