# Native Axum Transport Tests

Native integration tests owned by `auctioneer-adapter-axum`. The transport fixtures
run Auctioneer's fanout against real local TCP servers through
`edgezero_adapter_axum::outbound::AxumOutboundClient`. Fixture-only dependencies
and Tokio socket features are development dependencies of the adapter, not
production dependencies or a separate workspace package.

## Coverage

- `cancellation.rs` exercises the per-call bridge and observes connection
  closure when an outbound call loses its timeout race.
- `wave_transport.rs` exercises the bounded wave scheduler: cancellation
  after response headers, independent sibling deadlines, original slot ordering,
  streaming body limits, compressed and decoded gzip limits, invalid gzip EOF,
  and reuse of the stream permits after completion or refusal.
- `privacy_wire.rs` boots with the shipped envelope and timer, then sends a real
  auction through the handler, config loader, local bidder adapter and outbound
  client. Deny and allow controls check body disclosure, preserved context and
  signals, header isolation and stream-permit release.

Transport-only fixtures bind ephemeral loopback ports, with bounded waits. The
privacy fixture requires `insecure-bidders` and binds the shipped local bidder's
loopback port (8787); it fails rather than skips when that port is occupied.
A canned HTTP client can check orchestration results but cannot show whether the real transport closes
a connection or stops reading an oversized stream. These tests observe those
effects from the server side.

## Ownership And Scope

These are cross-component native transport checks, so they live in the adapter's
`tests/` directory rather than in portable library modules. The native test
workflow explicitly selects both transport binaries. Other tests in this directory
cover bootstrap independently, and the privacy wire probe has its own guarded
workflow step. There is no additional transport-test package.

The transport-only suite calls `auctioneer_core::host_primitives::EdgeZeroFanout`
with the real EdgeZero client and a local Tokio-backed test timer. It does not launch the full
Auctioneer server, exercise its bootstrap or prove the binary's timer wiring. The
privacy wire probe exercises the adapter's envelope and timer with the handler,
but not inbound Axum routing or the server executable. None of these tests
qualify Cloudflare, Fastly or Spin behavior.

## Run

```sh
cargo test --locked -p auctioneer-adapter-axum --test cancellation --test wave_transport
cargo test --locked -p auctioneer-adapter-axum --features insecure-bidders --test privacy_wire
```

Run on a native host that permits local TCP listeners. The fixtures do not need
external services, provider accounts or credentials.
