# Universal image that can build most things Rust
FROM phusion/baseimage

# stable|beta|nightly
ARG RUST_TOOLCHAIN=stable
ENV RUSTUP_HOME=/tmp/rustup 
ENV RUSTUP_BIN=$RUSTUP_HOME/toolchains/${RUST_TOOLCHAIN}-x86_64-unknown-linux-gnu/bin

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends software-properties-common \
			python-software-properties wget pkg-config build-essential ca-certificates curl clang libclang-dev \
			git libssl-dev gcc sudo vim libpq-dev

RUN mkdir /tmp/rustup && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain ${RUST_TOOLCHAIN}

ADD . /code
WORKDIR /code

EXPOSE 3000
ENV PATH=$RUSTUP_BIN:$PATH

RUN cargo build --release
CMD cargo run --release -- --bind 0.0.0.0
