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

Account login is delegated to monoes.me via OAuth 2.0 (authorization code +
PKCE) — this relay is a confidential client and never sees a password. Set
these env vars before starting:

- `MONOES_BASE_URL` — the monoes.me origin, e.g. `https://monoes.me`
- `MONOES_OAUTH_CLIENT_ID` / `MONOES_OAUTH_CLIENT_SECRET` — from a one-time
  `POST {MONOES_BASE_URL}/api/auth/oauth2/register` call (see the OAuth
  integration plan for the exact request body)
- `RELAY_PUBLIC_URL` — this relay's externally-reachable origin, used to build
  the `redirect_uri`
- `RELAY_ALLOWED_RETURN_ORIGINS` — comma-separated allowlist of web-app
  origins the login flow is permitted to redirect back to
- `JWT_SECRET` — signing key for the relay's own session JWTs (unrelated to
  the OAuth client secret above)

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
