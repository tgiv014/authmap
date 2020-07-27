FROM rust:1.40 as builder
RUN apt-get update
RUN apt-get install musl-tools -y
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /usr/src/authmap
COPY . .
RUN RUSTFLAGS=-Clinker=musl-gcc cargo install --path . --target=x86_64-unknown-linux-musl

FROM alpine:3.7
# This assumes you have pulled down the GeoLite2 Database
# It may be a good idea to force the user to provide this file via bind mount
COPY GeoLite2-City.mmdb /etc/authmap/GeoLite2-City.mmdb
COPY --from=builder /usr/local/cargo/bin/authmap /usr/local/bin/authmap
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/authmap"]
