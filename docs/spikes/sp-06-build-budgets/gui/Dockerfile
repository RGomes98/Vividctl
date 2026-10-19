# Ubuntu 24.04: GTK 4.14 and libadwaita 1.5, the compatibility floor of vivid-gui.
FROM ubuntu:24.04
ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config curl ca-certificates time \
        libgtk-4-dev libadwaita-1-dev \
    && rm -rf /var/lib/apt/lists/*
# The product pins `stable`, so measure with the current stable toolchain.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /spike
