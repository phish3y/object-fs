#!/bin/bash

mkdir -p target
g++ -Wall objectfs.cpp -laws-cpp-sdk-s3 -laws-cpp-sdk-core -lfuse3 -o target/objectfs