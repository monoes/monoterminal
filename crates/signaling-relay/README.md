# signaling-relay

A minimal, in-memory WebSocket signaling relay for MONOTERMINAL's WebRTC P2P feature. It pairs a
daemon (registered under a `peer_id`) with a browser client that wants to reach it, then blindly
forwards `offer`/`answer`/`ice_candidate` JSON messages between the two sockets so they can
establish a direct WebRTC `PeerConnection`. Once the DataChannel is up, this relay is no longer
involved — it never sees terminal traffic and holds no persistent state.

## Run locally

```bash
cargo run -p monoterminal-signaling-relay -- --bind-addr 0.0.0.0:9000
```

`--bind-addr` defaults to `0.0.0.0:9000` if omitted.

## Deploy via Docker

Build from the workspace root (the Dockerfile needs the full workspace as build context):

```bash
docker build -f crates/signaling-relay/Dockerfile -t signaling-relay .
docker run -p 9000:9000 signaling-relay
```

## Deploy via systemd

```bash
cargo build --release -p monoterminal-signaling-relay
sudo cp target/release/signaling-relay /usr/local/bin/signaling-relay
sudo cp crates/signaling-relay/signaling-relay.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now signaling-relay
```
