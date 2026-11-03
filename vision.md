# Auctioneer Product Vision

Updated: 2026-11-02

Auctioneer is an OpenRTB-native auction engine for publishers and publisher-serving
platforms. The product goal is one Rust implementation that can run as an
independent bidding server or as an in-binary auction module inside
[IAB Tech Lab Trusted Server](https://github.com/IABTechLab/trusted-server).

Publishers should be able to access the demand they need, control what data
leaves their environment, and choose where auctions run without maintaining
different auction implementations for each deployment model.

## OpenRTB First

OpenRTB is the primary request/response contract, starting with the project's
current OpenRTB 2.6 scope. Normalization, validation, privacy decisions, bidder
transformation, bounded fanout, and bid responses belong to the shared engine.
Supporting another protocol version requires an explicit compatibility design;
this vision does not silently add OpenRTB 2.5 conversion.

Broad bidder coverage and ecosystem interoperability make the engine useful
to publishers. Compatibility work must not weaken intentional security and
privacy protections or turn the product into an implementation copy. Required
auction extensions retain their documented contracts alongside the core
OpenRTB request/response model.

## Two Delivery Modes

| Mode                  | Publisher or platform experience                                                                                                                                                                                                       |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Standalone server** | Deploy Auctioneer and submit OpenRTB requests over HTTP. EdgeZero supplies host integration across supported native and WASM targets.                                                                                                      |
| **Embedded module**   | Link Auctioneer's Rust libraries into the Trusted Server binary and invoke the shared auction engine directly. No separate Auctioneer process, sidecar, loopback HTTP call, or external Auctioneer service is required for the auction invocation. |

The embedded mode is a first-class product goal, not merely an HTTP client
integration. Trusted Server assembles publisher context and invokes Auctioneer in
the same process. Auctioneer handles its bidder auction and returns typed results
for Trusted Server's wider orchestration and delivery flow. Bidder calls, and
optional external cache/store services, still cross network boundaries.

Both modes must use the same normalization and auction-policy implementation.
They may have different host bridges, but must not develop separate bidder,
consent, or targeting behavior. Equivalent inputs and policies should produce
equivalent results, apart from explicitly documented transport metadata.

Trusted Server already documents an auction-provider orchestration model and
a remote auction-service path. Directly embedding Auctioneer is this project's
integration goal, not a claim that the upstream project has implemented it.
The integration specification must confirm how a local engine participates
in that orchestration rather than assume the existing remote-provider API
accepts in-process results unchanged. See its
[auction orchestration guide](https://iabtechlab.github.io/trusted-server/guide/auction-orchestration.html).

## Publisher Control and Safe Integration

Trusted Server owns its ingress, publisher authentication/context, consent
collection, configuration provenance, and wider ad-serving workflow. Auctioneer
owns validation and privacy-safe bidder projection within its auction. The
integration must make account resolution, identity-source precedence, bidder
ownership, deadline propagation, resource reservations, and result mapping
explicit. It must avoid sending the same bidder through both local Auctioneer and
a remote provider unintentionally.

An in-process caller is trusted to supply host services and configuration;
it is not exempt from consent enforcement, input validation, outbound safety,
or resource accounting. A CMP can collect choices without replacing Auctioneer's
per-bidder enforcement. The host bridge must supply the existing engine
contracts, not bypass them to make embedding convenient.

Operators can enable or disable shipped, baked-eligible bidders through
validated runtime configuration without rebuilding. Adding adapter code or
changing baked bidder metadata requires a reviewed rebuild. The product does
not require runtime plugins or executable code downloads.

## Delivery Priorities

The [approved roadmap](docs/roadmap.md) preserves the core auction, privacy,
user-sync, and diagnostic-hardening contracts and sets these priorities:

- Preserve the delivered controlled-native flows for all 14 priority bidders,
  with truthful capability tracking and explicit resource/guest limitations.
- Deliver direct in-binary Trusted Server integration for selected banner/display
  flows, then hosted acceptance and a limited-bidder publisher pilot.
- Expand bidder coverage through reviewed demand-led waves after that test path;
  reconcile residual full-catalog migration separately.
- Deliver shared parity foundations and complete video support, including stored
  requests/accounts, currency/multibid/identity, caching, and long-form/ad-pod workflows.
- Complete remaining FPD, auction-value rules, AMP/native, privacy/transparency,
  compression, extensibility, observability, and browser privacy compatibility.

Trusted Server embedding needs a separately reviewed contract and plan. It can
use the existing library entry point and selected bidders without waiting for
the complete catalog or video support. Existing signed user sync, bounded
diagnostic projection, and controlled-native bidder delivery do not establish
embedding completion, worst-case resource certification, or hosted readiness.

A publisher does not need every bidder or media format to begin testing.
Diagnostic qualification is the local/staging hardening checkpoint, not a live
publisher pilot. Earlier functional tests can use the existing auction/privacy/sync
foundation. Trusted Server embedding owns in-binary integration and host-flow
acceptance; publisher-pilot work owns hosted execution, selected-deployment
qualification, and live rollout/rollback. Native, AMP, or other required features
remain dependencies of their owning capabilities rather than implied support.

## Success Measures

- A publisher can execute an auction, receive valid bids, apply targeting,
  render an eligible creative, and verify the configured demand path.
- An embedded host can invoke the same engine without an Auctioneer service hop,
  with conformance tests demonstrating policy and result equivalence.
- Consent-denied data does not reach bidders, and privacy/reference-data
  failures produce the documented conservative outcomes.
- Latency, timeouts, resource use, bidder errors, and config failures are
  measurable against explicit publisher/deployment budgets, not unsupported
  performance claims.
- Bidder coverage and feature parity are separately visible; registration,
  compilation, or local tests alone do not imply live integration readiness.
- A publisher can roll out a selected bidder set, compare it with its existing
  integration, and roll back without changing the auction implementation.

## Product Boundary

Auctioneer is not a CMP, browser header-bidding replacement, ad server, video
player, SSAI service, identity graph, or durable auction-history database.
It integrates with those systems where the publisher workflow needs them.
Integrating a compatible creative cache does not require building a new cache
product. Technical privacy enforcement is not a claim of legal certification.

The [architecture](docs/architecture.md) records current capabilities and
ownership. The [master design](docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md)
and capability specifications define exact behavior. This document describes
the product direction, not implementation completion or deployment approval.
