FROM debian:stable-slim

RUN apt-get update && apt-get install -y \
    cmake \
    curl \
    fuse \
    g++ \
    git \
    meson \
    libcurl4-openssl-dev \ 
    libssl-dev \
    zlib1g-dev

WORKDIR /src
RUN git clone --recurse-submodules https://github.com/aws/aws-sdk-cpp
WORKDIR /src/aws-sdk-cpp
RUN mkdir -p build
WORKDIR /src/aws-sdk-cpp/build
RUN cmake .. -DCMAKE_BUILD_TYPE=Debug -DBUILD_ONLY="s3"
RUN cmake --build . --config=Debug
RUN cmake --install . --config=Debug

WORKDIR /src
RUN mkdir -p fuse
WORKDIR /src/fuse
RUN curl -L -o fuse-3.16.2.tar.gz https://github.com/libfuse/libfuse/releases/download/fuse-3.16.2/fuse-3.16.2.tar.gz
RUN tar xzf fuse-3.16.2.tar.gz
WORKDIR /src/fuse/fuse-3.16.2
RUN mkdir -p build
WORKDIR /src/fuse/fuse-3.16.2/build
RUN meson setup ..
RUN ninja
RUN ninja install

WORKDIR /src
RUN git clone https://github.com/gabime/spdlog.git
WORKDIR /src/spdlog
RUN mkdir -p build
WORKDIR /src/spdlog/build
RUN cmake ..
RUN cmake --build .
RUN cmake --install .

WORKDIR /src
RUN mkdir -p objectfs
WORKDIR /src/objectfs
COPY objectfs.cpp .

RUN g++ -Wall objectfs.cpp -laws-cpp-sdk-s3 -laws-cpp-sdk-core -lfuse3 -o /usr/local/bin/objectfs

ENV LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/usr/local/lib:/usr/local/lib/aarch64-linux-gnu
RUN mkdir -p /tmp/fuse
ENTRYPOINT [ "objectfs", "-s", "-f", "/tmp/fuse" ]