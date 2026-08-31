FROM rust:1-slim AS builder

WORKDIR /build
COPY . .

RUN cargo build --release -p monoterminal-signaling-relay

FROM debian:bookworm-slim

COPY --from=builder /build/target/release/signaling-relay /usr/local/bin/signaling-relay

EXPOSE 9000

ENTRYPOINT ["/usr/local/bin/signaling-relay"]
CMD ["--bind-addr", "0.0.0.0:9000"]
