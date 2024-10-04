#!/bin/bash

gcc -Wall fs.c `pkg-config fuse3 --cflags --libs` -o target/e_fs
