FROM rust:1.33-stretch

COPY . /usr/src/coordinator
WORKDIR /usr/src/coordinator

RUN /usr/local/cargo/bin/cargo build \
    && /usr/local/cargo/bin/cargo test --verbose
