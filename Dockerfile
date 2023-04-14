FROM rust:latest as builder
WORKDIR /usr/src/app
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive \
    apt-get install --no-install-recommends --assume-yes \
    protobuf-compiler libprotobuf-dev
COPY . .
COPY ./targe[t]/cach[e]/us[r]/ /usr/
RUN rm -rf ./target/cache
RUN CARGO_INCREMENTAL=0 cargo build --release --package mpc-recovery

FROM scratch as export-artifacts
COPY --from=builder /usr/src/app/target /usr/src/app/target
COPY --from=builder /usr/local/cargo/bin /usr/local/cargo/bin
COPY --from=builder /usr/local/cargo/git* /usr/local/cargo/git
COPY --from=builder /usr/local/cargo/.crate.toml* /usr/local/cargo/.crate.toml
COPY --from=builder /usr/local/cargo/.crate2.toml* /usr/local/cargo/.crate2.toml
COPY --from=builder /usr/local/cargo/registry/cache /usr/local/cargo/registry/cache
COPY --from=builder /usr/local/cargo/registry/index /usr/local/cargo/registry/index

FROM debian:buster-slim as runtime
RUN apt-get update && apt-get install --assume-yes libssl-dev
COPY --from=builder /usr/src/app/target/release/mpc-recovery /usr/local/bin/mpc-recovery
WORKDIR /usr/local/bin

ENTRYPOINT [ "mpc-recovery" ]
