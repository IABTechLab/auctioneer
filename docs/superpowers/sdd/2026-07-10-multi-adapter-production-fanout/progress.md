# SDD ledger — plan: docs/superpowers/plans/2026-07-10-multi-adapter-production-fanout.md

> Committed execution records live under `docs/superpowers/sdd/` with the project specs and plans.

> Historical commands below record the package names actually tested at those
> revisions. Transport fixtures now live under `crates/auctioneer-adapter-axum/tests/`;
> the current command is `cargo test --locked -p auctioneer-adapter-axum --test cancellation --test wave_transport`.

## Task Fanout-9: complete

- Implemented directly (body already fully specified; expansion gate satisfied).
- TDD: `runtime_host_alias_unknown_base_returns_malformed_host` verified RED
  (unknown base resolved Ok) before implementation, GREEN after; positive
  control `runtime_host_alias_known_base_is_accepted` green both sides.
- New module `crates/auctioneer-core/src/bidder_registry.rs` (registry singleton +
  `live_names()` moved out of handlers::auction — config layer no longer
  reaches into handler symbols); `validate_alias_bases` in
  `config/runtime.rs` Ok arm.
- Gate: fmt clean; clippy -D warnings on default AND insecure-bidders
  combos; full `cargo test -p auctioneer-core` green (incl. the 12
  runtime_config_failclosed rows). Test count +1 per the Fanout-8 gate row.

## Atomic EdgeZero pin move: complete (round-142 #1)

- Cargo.lock: docs/outbound-http-spec 12965da2 -> f3fe8da9 (the reviewed
  PR-275 head) in ONE commit with the full RequestContext migration:
  edgezero_core::proxy -> outbound::HttpClient (EdgeZeroProxyAdapter over
  OutboundRequest/OutboundResponse; EdgeZeroFanout takes HttpClient),
  ctx.request() -> ctx.headers()/ctx.extensions(), into_request() ->
  Result handled, proxy_handle() -> http_client(), conformance suite +
  production_fanout/axum_production_strategy mocks + Axum transport tests
  ported to OutboundHttpClient/AxumOutboundClient.
- Behavior carried by necessity: EdgeError::StoreExtraction { reason }
  mapped by the round-140 #4 total table (classify_store_extraction) —
  without it the five GDPR fanout fail-closed rows regressed to fail-open at the
  new pin. MissingBlob keeps the GDPR fanout degrade until allow_missing_config.
- Bridge header conversion is now fail-closed (OutboundRequest::header
  errors fail the call instead of silently dropping the header).
- Test stubs spell out ConfigStore::get_bounded via one shared
  include-only helper (tests/support/bounded_store.rs) — no new expect.
- Gate (all green, same tree): fmt; clippy -D warnings default /
  insecure-bidders / insecure-bidders+conformance-suite; cargo test
  -p auctioneer-core --features "insecure-bidders conformance-suite";
  cargo test -p auctioneer-axum-transport-tests; cargo test --workspace.

## bounded fanout synchronization task: complete (round-143)

- Pin f3fe8da9 -> 6feeae42 (reviewed PR-275 head); gate at the new pin:
  clippy -D warnings (default; insecure-bidders+conformance-suite), the
  conformance + auctioneer-core suites, the Axum transport tests — all green.
- Plan/spec corrected: BoundedFetch/send_all claims retracted (Fanout-2b
  marked UNIMPLEMENTED with the accurate per-call bridge baseline); one
  time architecture (application clock; Duration/TimeDomain wording
  deleted; Deadline::at_instant); real [capabilities] model (one common
  manifest, ingress-admission required, degraded rows optional, CI
  assertions per target); allow_missing_config bootstrap fully defined
  (RuntimeBootstrap type, data/runtime-bootstrap.toml, build.rs codegen,
  default false, build-time validation); TargetOutboundProfile descriptor
  - install path; spec stale sections superseded; expansion gate reworded.

## Task Fanout-1a: complete (round-143)

- TDD: the two re-asserted fail-closed rows
  (missing_blob_fails_closed_under_default_bootstrap,
  accepted_store_key_with_no_blob_fails_closed_under_default_bootstrap)
  verified RED against the pre-rule code, GREEN after.
- Bootstrap: data/runtime-bootstrap.toml (allow_missing_config = false),
  RuntimeBootstrapRaw/RuntimeSectionRaw (deny_unknown_fields, strict
  bool) in build_codegen.rs, emit_runtime_bootstrap in build.rs ->
  auctioneer_config::RUNTIME_BOOTSTRAP, RuntimeBootstrap in types.rs.
- Loader: classify_store_extraction_with(reason, field_path, allow) —
  MissingBlob degrades only under `true`, else 500; residual
  Internal/BadRequest/ServiceUnavailable arms carry fixed text (no
  upstream strings).
- Tests: per-variant class/exact-detail/status over all sixteen reasons
  (transient trio -> 503+Retry-After; malformed twelve -> 500), both
  bootstrap branches, config_error_logs_structured_allowlist_no_raw_text,
  four codegen strictness cases (checked-in file parses to false; unknown
  key / non-bool / missing key rejected).
- Gate (all green, same tree): fmt; clippy -D warnings default and
  insecure-bidders+conformance-suite; auctioneer-config codegen tests;
  auctioneer-core with insecure-bidders+conformance-suite; Axum transport
  tests; full workspace.
- NOT DONE: mutation kill — cargo-mutants is not installed here;
  recorded as an open acceptance item for Fanout-8, not claimed.

## Round-144 corrections: complete

- Fanout-1a privacy gaps closed: the absent [stores.config] binding takes the
  same immutable allow_missing_config rule as a missing blob (one
  `missing_config` fn); the non_exhaustive EdgeError remainder fails
  closed with fixed text; `classify_load` is the pure classification,
  unit-tested with canary-bearing outer errors (not_found / validation /
  internal / bad_request / service_unavailable) — no upstream text in
  `detail`. Integration: absent_store_binding_fails_closed_under_default_
  bootstrap; eight handler tests across four binaries + the conformance
  no-outbound row now bind a default config store (they exercised the
  seam they name, not the loader's answer to an unbound store).
- Docs: Cloudflare profile = [RawCompressed, GuestDecoded]; bootstrap is
  source-controlled policy only (harness override feature-gated); the
  capability table carries all declared rows; mutation kill moved to
  Fanout-8 in 1a's completion definition; spec status/Fastly/baseline
  refreshed; Fanout-2a expanded into six executable sub-tasks + the gated
  FINAL constants step (awaiting re-review before any 2a code).
- Gate (all green): fmt; clippy -D warnings x2; auctioneer-config codegen;
  auctioneer-core insecure-bidders+conformance-suite; Axum transport; full
  workspace.

## Round-145 corrections: complete

- Pin 6feeae42 -> 120dd2aa (feature-gating/adapter refactors upstream;
  consumed surface unchanged). Gate at the new pin: clippy -D warnings
  x2, auctioneer-core insecure-bidders+conformance-suite, Axum transport,
  full workspace — all green, no code change needed.
- Plan/spec: OutboundRequestSink is two-stage (reserve -> metadata ->
  validate(url, info) -> body -> commit; body closure never runs after a
  rejected URL); Fanout-2b gains the mandatory 3xx rejection line item
  (classify rejects every 3xx as BadServerResponse code 4 / seatnonbid
  100 without parsing; per-adapter no-contact proof); one precise
  definition of "concurrent" (dispatch-before-harvest; Fastly adds no
  complete-exchange concurrency or independent terminal observation);
  StreamGate expansion gains counting-waker A/B and reentrant-waker
  tests; supported_body_sources()/guest_decoder_memory_bound() trait
  methods struck in favor of TargetOutboundProfile. Roster +4 -> +115
  (108 with Fanout-2, 7 with Fanout-10). Fanout-2a.4 must not start until the
  two-stage sink is re-reviewed.
