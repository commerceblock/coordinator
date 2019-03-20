FROM rust:1.33-stretch

COPY . /usr/src/coordinator
WORKDIR /usr/src/coordinator

ENTRYPOINT ["/usr/local/cargo/bin/cargo"]
CMD ["test","--verbose"]
