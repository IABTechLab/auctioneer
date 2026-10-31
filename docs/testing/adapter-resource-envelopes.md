# Adapter Resource Envelopes

Generated from the canonical typed source audit. No remediation package is approved by this command. This is not a physical target measurement or fit sign-off.

Code baseline: `ac6d54112638e0576cf57cc6dde2333188e60530`. SDK: `f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`. Lock: `7f2710cf235c5b358354905cf6b64a892b2fd1058b0c97439c353d7ec0b26a7d`.

Features: `application`.

Compiler:

```text
rustc 1.98.1 (48a229cea 2026-09-01)
binary: rustc
commit-hash: 48a229ceaefd4985c50990b14116b6d856af0985
commit-date: 2026-09-01
host: aarch64-apple-darwin
release: 1.98.1
LLVM version: 22.1.8
```

The equation is the runtime's checked additive sum; receipts are already population-scaled. Headroom is a separate acceptance inequality. Pending platform facts are not numeric zero.

| Adapter    | Demand Bytes | Headroom Bytes |    Gap Bytes | Fit     |
| ---------- | -----------: | -------------: | -----------: | ------- |
| axum       |   2533631672 |        pending |      pending | blocked |
| cloudflare | 138048496272 |        8000000 | 137928496272 | blocked |
| fastly     |  23589646360 |        8000000 |  23469646360 | blocked |
| spin       |  23589646360 |        8388608 |  23463817240 | blocked |

## axum

Profile `axum`; dispatch `axum`; layout `native_control`; reporter `aarch64-macos-64bit`; derived compiler target `native`.

Profile digest: `a9d6affb82d884a5f51e8e049c3cd9615febd2415e8f49480f40965838af7644`.

| Term                     |      Bytes | Allocation Owner                                                                     | Acquire                                                                     | Release                                                                                      | Overlap                                                                                                        |
| ------------------------ | ---------: | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `baseline`               |  144769024 | [Auctioneer target/core maintainers](../../crates/auctioneer-core/src/bidder_registry.rs)    | Cold initialization of defaults, registry and singleton state               | Instance or execution teardown                                                               | Existing startup reserves remain additive with admitted work                                                   |
| `gvl`                    |    3584000 | [Auctioneer privacy maintainer](../../crates/auctioneer-privacy/src/gvl.rs)                  | GvlStore::get parses and caches a version; requests clone a handle          | Last cache or request handle, including requests pinning evicted versions                    | Cache entries plus distinct auction-held and auxiliary-held versions; handles are not duplicate parsed lists   |
| `auctions`               | 1309755768 | [Auctioneer core/exchange maintainers](../../crates/auctioneer-core/src/handlers/auction.rs) | Auction admission and all six phase receipts before config extraction       | Serialization completion or cancellation drops the work receipts                             | Config, decoded ingress, privacy projections, outbound requests and completed results can coexist              |
| `auction_egress`         |    6299648 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/handlers/auction.rs)   | Egress reservation before response serialization                            | Terminal response completion or cancellation, not work completion                            | Serialized body, provider handoff copies and transmit state can outlive auction work                           |
| `auxiliary`              |  536879104 | [Auctioneer core auxiliary-route maintainer](../../crates/auctioneer-core/src/ingress.rs)    | Auxiliary route admission/work reservation or independent probe-config work | Route completion versus probe config resolution/render/publication; counters are independent | Auxiliary work and max(auxiliary, probe minimum) config work coexist with output handoff                       |
| `terminal_egress`        |    1769472 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/ingress.rs)            | Terminal/static output reservation, including detached head refusals        | Response completion or cancellation                                                          | Refusal output and transmit state are independent of auction and auxiliary work                                |
| `probe_output`           |      90112 | [Auctioneer core health maintainer](../../crates/auctioneer-core/src/ingress.rs)             | Independent probe-output reservation, including preboot responses           | Output completion; the cached verdict remains resident independently                         | Live probe output population plus one cached verdict, not probe-config work count                              |
| `pre_admission_parser`   |  487784448 | [EdgeZero adapter maintainer/provider](../../crates/auctioneer-runtime/src/budget.rs)    | Host/framing/head parsing before Auctioneer route admission                     | Host/parser cleanup, including rejected and cancelled input                                  | No Auctioneer admission counter bounds the pre-admission population; outbound connections are not inbound requests |
| `body_streams`           |   42700096 | [Auctioneer outbound/target maintainers](../../crates/auctioneer-core/src/outbound/wave.rs)  | Global wave stream lease before dispatch and decode                         | Wave settlement, EOF or cancellation                                                         | Completed response slots survive stream release across waves and remain in auction response funding            |
| `catalog_resident`       |          0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental generated catalog/validator residency at initialization         | Instance or execution teardown                                                               | Zero incremental descriptor does not remove existing registry residency funded by baseline                     |
| `catalog_initialization` |          0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental catalog initialization scratch, including validation failure    | Initialization completion or error cleanup                                                   | Initialization remains an additive term even when no auction is currently executing                            |

Artifact proof: pending. Target ABI bound: pending. Known owner ABI bounds: 0/11. The charged population is not evidence of provider ingress enforcement.

### Components

These are explanatory, already-scaled ledger entries, not additional summands. Nested phase/subcomponent rows must not be added to their parents.

| Component            |     Bytes | Above 5% Of Demand |
| -------------------- | --------: | ------------------ |
| `auction_config`     |  61206528 | no                 |
| `auction_ingress`    |  31688904 | no                 |
| `auction_privacy`    |  29361728 | no                 |
| `auction_projection` | 730590512 | yes                |
| `auction_outbound`   |   3521536 | no                 |
| `auction_response`   | 453386560 | yes                |
| `runtime_config`     |  61206528 | no                 |
| `request_side`       | 762279416 | yes                |
| `call_side`          | 431733760 | yes                |
| `bookkeeping`        |   6553600 | no                 |
| `debug`              |  18620736 | no                 |
| `assembly`           |   3145728 | no                 |
| `aux_config`         |  50999296 | no                 |
| `aux_body`           |   2424832 | no                 |
| `aux_query`          |    155648 | no                 |
| `aux_input_headers`  |     49152 | no                 |
| `aux_cookie`         |     98304 | no                 |
| `aux_signing`        |   1060864 | no                 |
| `aux_nodes`          |  78643200 | no                 |
| `aux_privacy`        |  21233664 | no                 |
| `aux_chooser`        |    794624 | no                 |
| `aux_urls`           |    196608 | no                 |
| `aux_retained_urls`  |    524288 | no                 |
| `aux_output`         |   1105920 | no                 |
| `aux_declared_work`  | 268435456 | yes                |
| `aux_transmit`       |      8192 | no                 |
| `aux_handoff`        | 268435456 | yes                |
| `aux_routes`         | 268439552 | yes                |
| `probe_config`       | 268439552 | yes                |

### Next Proofs

| Term                     | Disposition | Owner                                  | Required Proof                                                                                                                                  |
| ------------------------ | ----------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `baseline`               | unchanged   | Auctioneer target/core maintainers         | Measure cold and warm target residency, with initialization and failure paths identified separately                                             |
| `gvl`                    | unchanged   | Auctioneer privacy maintainer              | Prove target parsed-list bounds and eviction with multiple distinct request-pinned versions                                                     |
| `auctions`               | unchanged   | Auctioneer core/exchange maintainers       | Bound each phase on the target ABI at two sizes and unequal populations, including late failure and cancellation                                |
| `auction_egress`         | unchanged   | Auctioneer core/target maintainers         | Exercise a slow terminal handoff and cancellation while new auctions are admitted                                                               |
| `auxiliary`              | unchanged   | Auctioneer core auxiliary-route maintainer | Prove preboot/App-retained config defaults first, then cookie, signing, privacy, chooser, output and host-copy overlap on every auxiliary route |
| `terminal_egress`        | unchanged   | Auctioneer core/target maintainers         | Exercise preboot and saturated-class refusal output through actual target handoff                                                               |
| `probe_output`           | unchanged   | Auctioneer core health maintainer          | Measure cached verdict plus live preboot/boot probe bodies at concurrent terminal completion                                                    |
| `pre_admission_parser`   | unchanged   | EdgeZero adapter maintainer/provider   | Publish and negatively test provider ingress population, framing, parser and host-storage bounds at the memory scope                            |
| `body_streams`           | unchanged   | Auctioneer outbound/target maintainers     | Drive multiple waves with retained completed results, decoder windows and cancellation on the guest ABI                                         |
| `catalog_resident`       | unchanged   | Auctioneer catalog maintainer              | Measure real generated descriptors against the matched pre-catalog baseline without installation in the pure profile test                       |
| `catalog_initialization` | unchanged   | Auctioneer catalog maintainer              | Measure successful and failing catalog initialization separately from retained descriptors on the target ABI                                    |

### External Blockers

| Fact                                     | Owner                                       | Next Verification                                                                                              |
| ---------------------------------------- | ------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `unknown_memory_ceiling`                 | AG/operator and EdgeZero adapter maintainer | Publish operator ceiling and memory scope through the SDK and exercise configured overflow independently       |
| `unknown_inbound_request_population`     | EdgeZero adapter maintainer/provider        | Publish or enforce a scope-bound inbound population; do not reuse the outbound connection limit                |
| `unknown_host_ingress_memory_accounting` | EdgeZero adapter maintainer/provider        | Publish and test host ingress/framing allocation ownership and bounds including cancelled and refused requests |

## cloudflare

Profile `cloudflare`; dispatch `cloudflare`; layout `host_native_substitute`; reporter `aarch64-macos-64bit`; derived compiler target `wasm32-unknown-unknown`.

Profile digest: `311f62be9707380f9068ae5b4017b7e946a1d8a839cfae7b7c58708679c5a9cd`.

| Term                     |        Bytes | Allocation Owner                                                                     | Acquire                                                                     | Release                                                                                      | Overlap                                                                                                        |
| ------------------------ | -----------: | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `baseline`               |     27328512 | [Auctioneer target/core maintainers](../../crates/auctioneer-core/src/bidder_registry.rs)    | Cold initialization of defaults, registry and singleton state               | Instance or execution teardown                                                               | Existing startup reserves remain additive with admitted work                                                   |
| `gvl`                    |      7168000 | [Auctioneer privacy maintainer](../../crates/auctioneer-privacy/src/gvl.rs)                  | GvlStore::get parses and caches a version; requests clone a handle          | Last cache or request handle, including requests pinning evicted versions                    | Cache entries plus distinct auction-held and auxiliary-held versions; handles are not duplicate parsed lists   |
| `auctions`               | 117533598864 | [Auctioneer core/exchange maintainers](../../crates/auctioneer-core/src/handlers/auction.rs) | Auction admission and all six phase receipts before config extraction       | Serialization completion or cancellation drops the work receipts                             | Config, decoded ingress, privacy projections, outbound requests and completed results can coexist              |
| `auction_egress`         |    453009408 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/handlers/auction.rs)   | Egress reservation before response serialization                            | Terminal response completion or cancellation, not work completion                            | Serialized body, provider handoff copies and transmit state can outlive auction work                           |
| `auxiliary`              |  19327385600 | [Auctioneer core auxiliary-route maintainer](../../crates/auctioneer-core/src/ingress.rs)    | Auxiliary route admission/work reservation or independent probe-config work | Route completion versus probe config resolution/render/publication; counters are independent | Auxiliary work and max(auxiliary, probe minimum) config work coexist with output handoff                       |
| `terminal_egress`        |        48640 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/ingress.rs)            | Terminal/static output reservation, including detached head refusals        | Response completion or cancellation                                                          | Refusal output and transmit state are independent of auction and auxiliary work                                |
| `probe_output`           |       122880 | [Auctioneer core health maintainer](../../crates/auctioneer-core/src/ingress.rs)             | Independent probe-output reservation, including preboot responses           | Output completion; the cached verdict remains resident independently                         | Live probe output population plus one cached verdict, not probe-config work count                              |
| `pre_admission_parser`   |      1428480 | [EdgeZero adapter maintainer/provider](../../crates/auctioneer-runtime/src/budget.rs)    | Host/framing/head parsing before Auctioneer route admission                     | Host/parser cleanup, including rejected and cancelled input                                  | No Auctioneer admission counter bounds the pre-admission population; outbound connections are not inbound requests |
| `body_streams`           |    698405888 | [Auctioneer outbound/target maintainers](../../crates/auctioneer-core/src/outbound/wave.rs)  | Global wave stream lease before dispatch and decode                         | Wave settlement, EOF or cancellation                                                         | Completed response slots survive stream release across waves and remain in auction response funding            |
| `catalog_resident`       |            0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental generated catalog/validator residency at initialization         | Instance or execution teardown                                                               | Zero incremental descriptor does not remove existing registry residency funded by baseline                     |
| `catalog_initialization` |            0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental catalog initialization scratch, including validation failure    | Initialization completion or error cleanup                                                   | Initialization remains an additive term even when no auction is currently executing                            |

Artifact proof: pending. Target ABI bound: pending. Known owner ABI bounds: 0/11. The charged population is not evidence of provider ingress enforcement.

### Components

These are explanatory, already-scaled ledger entries, not additional summands. Nested phase/subcomponent rows must not be added to their parents.

| Component            |       Bytes | Above 5% Of Demand |
| -------------------- | ----------: | ------------------ |
| `auction_config`     |  1648742400 | no                 |
| `auction_ingress`    |  1942444632 | no                 |
| `auction_privacy`    |   264245952 | no                 |
| `auction_projection` | 89345971128 | yes                |
| `auction_outbound`   |   106125312 | no                 |
| `auction_response`   | 24226069440 | yes                |
| `runtime_config`     |  1648742400 | no                 |
| `request_side`       | 91288415760 | yes                |
| `call_side`          | 23663695872 | yes                |
| `bookkeeping`        |   610271232 | no                 |
| `debug`              |    58227648 | no                 |
| `assembly`           |   150994944 | no                 |
| `aux_config`         |  2157494272 | no                 |
| `aux_body`           |     9699328 | no                 |
| `aux_query`          |      622592 | no                 |
| `aux_input_headers`  |      196608 | no                 |
| `aux_cookie`         |      393216 | no                 |
| `aux_signing`        |     4243456 | no                 |
| `aux_nodes`          |  3334471680 | no                 |
| `aux_privacy`        |    84934656 | no                 |
| `aux_chooser`        |     3178496 | no                 |
| `aux_urls`           |      786432 | no                 |
| `aux_retained_urls`  |     2097152 | no                 |
| `aux_output`         |     4423680 | no                 |
| `aux_declared_work`  |  6442450944 | no                 |
| `aux_transmit`       |       32768 | no                 |
| `aux_handoff`        | 12884901888 | yes                |
| `aux_routes`         |  9663692800 | yes                |
| `probe_config`       |  9663692800 | yes                |

### Next Proofs

| Term                     | Disposition | Owner                                  | Required Proof                                                                                                                                  |
| ------------------------ | ----------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `baseline`               | unchanged   | Auctioneer target/core maintainers         | Measure cold and warm target residency, with initialization and failure paths identified separately                                             |
| `gvl`                    | unchanged   | Auctioneer privacy maintainer              | Prove target parsed-list bounds and eviction with multiple distinct request-pinned versions                                                     |
| `auctions`               | unchanged   | Auctioneer core/exchange maintainers       | Bound each phase on the target ABI at two sizes and unequal populations, including late failure and cancellation                                |
| `auction_egress`         | unchanged   | Auctioneer core/target maintainers         | Exercise a slow terminal handoff and cancellation while new auctions are admitted                                                               |
| `auxiliary`              | unchanged   | Auctioneer core auxiliary-route maintainer | Prove preboot/App-retained config defaults first, then cookie, signing, privacy, chooser, output and host-copy overlap on every auxiliary route |
| `terminal_egress`        | unchanged   | Auctioneer core/target maintainers         | Exercise preboot and saturated-class refusal output through actual target handoff                                                               |
| `probe_output`           | unchanged   | Auctioneer core health maintainer          | Measure cached verdict plus live preboot/boot probe bodies at concurrent terminal completion                                                    |
| `pre_admission_parser`   | unchanged   | EdgeZero adapter maintainer/provider   | Publish and negatively test provider ingress population, framing, parser and host-storage bounds at the memory scope                            |
| `body_streams`           | unchanged   | Auctioneer outbound/target maintainers     | Drive multiple waves with retained completed results, decoder windows and cancellation on the guest ABI                                         |
| `catalog_resident`       | unchanged   | Auctioneer catalog maintainer              | Measure real generated descriptors against the matched pre-catalog baseline without installation in the pure profile test                       |
| `catalog_initialization` | unchanged   | Auctioneer catalog maintainer              | Measure successful and failing catalog initialization separately from retained descriptors on the target ABI                                    |

### External Blockers

| Fact                                     | Owner                                | Next Verification                                                                                              |
| ---------------------------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `unknown_inbound_request_population`     | EdgeZero adapter maintainer/provider | Publish or enforce a scope-bound inbound population; do not reuse the outbound connection limit                |
| `unknown_host_ingress_memory_accounting` | EdgeZero adapter maintainer/provider | Publish and test host ingress/framing allocation ownership and bounds including cancelled and refused requests |

## fastly

Profile `fastly`; dispatch `fastly`; layout `host_native_substitute`; reporter `aarch64-macos-64bit`; derived compiler target `wasm32-wasip1`.

Profile digest: `8b48f9e23c5f65beac54753e29f8de66873cff8ccd617ee008fceb193581ba9c`.

| Term                     |       Bytes | Allocation Owner                                                                     | Acquire                                                                     | Release                                                                                      | Overlap                                                                                                        |
| ------------------------ | ----------: | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `baseline`               |    27328512 | [Auctioneer target/core maintainers](../../crates/auctioneer-core/src/bidder_registry.rs)    | Cold initialization of defaults, registry and singleton state               | Instance or execution teardown                                                               | Existing startup reserves remain additive with admitted work                                                   |
| `gvl`                    |     3072000 | [Auctioneer privacy maintainer](../../crates/auctioneer-privacy/src/gvl.rs)                  | GvlStore::get parses and caches a version; requests clone a handle          | Last cache or request handle, including requests pinning evicted versions                    | Cache entries plus distinct auction-held and auxiliary-held versions; handles are not duplicate parsed lists   |
| `auctions`               | 19588933144 | [Auctioneer core/exchange maintainers](../../crates/auctioneer-core/src/handlers/auction.rs) | Auction admission and all six phase receipts before config extraction       | Serialization completion or cancellation drops the work receipts                             | Config, decoded ingress, privacy projections, outbound requests and completed results can coexist              |
| `auction_egress`         |    50335744 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/handlers/auction.rs)   | Egress reservation before response serialization                            | Terminal response completion or cancellation, not work completion                            | Serialized body, provider handoff copies and transmit state can outlive auction work                           |
| `auxiliary`              |  3221233664 | [Auctioneer core auxiliary-route maintainer](../../crates/auctioneer-core/src/ingress.rs)    | Auxiliary route admission/work reservation or independent probe-config work | Route completion versus probe config resolution/render/publication; counters are independent | Auxiliary work and max(auxiliary, probe minimum) config work coexist with output handoff                       |
| `terminal_egress`        |        9216 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/ingress.rs)            | Terminal/static output reservation, including detached head refusals        | Response completion or cancellation                                                          | Refusal output and transmit state are independent of auction and auxiliary work                                |
| `probe_output`           |       90112 | [Auctioneer core health maintainer](../../crates/auctioneer-core/src/ingress.rs)             | Independent probe-output reservation, including preboot responses           | Output completion; the cached verdict remains resident independently                         | Live probe output population plus one cached verdict, not probe-config work count                              |
| `pre_admission_parser`   |      238080 | [EdgeZero adapter maintainer/provider](../../crates/auctioneer-runtime/src/budget.rs)    | Host/framing/head parsing before Auctioneer route admission                     | Host/parser cleanup, including rejected and cancelled input                                  | No Auctioneer admission counter bounds the pre-admission population; outbound connections are not inbound requests |
| `body_streams`           |   698405888 | [Auctioneer outbound/target maintainers](../../crates/auctioneer-core/src/outbound/wave.rs)  | Global wave stream lease before dispatch and decode                         | Wave settlement, EOF or cancellation                                                         | Completed response slots survive stream release across waves and remain in auction response funding            |
| `catalog_resident`       |           0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental generated catalog/validator residency at initialization         | Instance or execution teardown                                                               | Zero incremental descriptor does not remove existing registry residency funded by baseline                     |
| `catalog_initialization` |           0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental catalog initialization scratch, including validation failure    | Initialization completion or error cleanup                                                   | Initialization remains an additive term even when no auction is currently executing                            |

Artifact proof: pending. Target ABI bound: pending. Known owner ABI bounds: 0/11. The charged population is not evidence of provider ingress enforcement.

### Components

These are explanatory, already-scaled ledger entries, not additional summands. Nested phase/subcomponent rows must not be added to their parents.

| Component            |       Bytes | Above 5% Of Demand |
| -------------------- | ----------: | ------------------ |
| `auction_config`     |   274790400 | no                 |
| `auction_ingress`    |   323740772 | no                 |
| `auction_privacy`    |    44040992 | no                 |
| `auction_projection` | 14890995188 | yes                |
| `auction_outbound`   |    17687552 | no                 |
| `auction_response`   |  4037678240 | yes                |
| `runtime_config`     |   274790400 | no                 |
| `request_side`       | 15214735960 | yes                |
| `call_side`          |  3943949312 | yes                |
| `bookkeeping`        |   101711872 | no                 |
| `debug`              |     9704608 | no                 |
| `assembly`           |    25165824 | no                 |
| `aux_config`         |   539373568 | no                 |
| `aux_body`           |     2424832 | no                 |
| `aux_query`          |      155648 | no                 |
| `aux_input_headers`  |       49152 | no                 |
| `aux_cookie`         |       98304 | no                 |
| `aux_signing`        |     1060864 | no                 |
| `aux_nodes`          |   833617920 | no                 |
| `aux_privacy`        |    21233664 | no                 |
| `aux_chooser`        |      794624 | no                 |
| `aux_urls`           |      196608 | no                 |
| `aux_retained_urls`  |      524288 | no                 |
| `aux_output`         |     1105920 | no                 |
| `aux_declared_work`  |  1610612736 | yes                |
| `aux_transmit`       |        8192 | no                 |
| `aux_handoff`        |  1610612736 | yes                |
| `aux_routes`         |  1610616832 | yes                |
| `probe_config`       |  1610616832 | yes                |

### Next Proofs

| Term                     | Disposition | Owner                                  | Required Proof                                                                                                                                  |
| ------------------------ | ----------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `baseline`               | unchanged   | Auctioneer target/core maintainers         | Measure cold and warm target residency, with initialization and failure paths identified separately                                             |
| `gvl`                    | unchanged   | Auctioneer privacy maintainer              | Prove target parsed-list bounds and eviction with multiple distinct request-pinned versions                                                     |
| `auctions`               | unchanged   | Auctioneer core/exchange maintainers       | Bound each phase on the target ABI at two sizes and unequal populations, including late failure and cancellation                                |
| `auction_egress`         | unchanged   | Auctioneer core/target maintainers         | Exercise a slow terminal handoff and cancellation while new auctions are admitted                                                               |
| `auxiliary`              | unchanged   | Auctioneer core auxiliary-route maintainer | Prove preboot/App-retained config defaults first, then cookie, signing, privacy, chooser, output and host-copy overlap on every auxiliary route |
| `terminal_egress`        | unchanged   | Auctioneer core/target maintainers         | Exercise preboot and saturated-class refusal output through actual target handoff                                                               |
| `probe_output`           | unchanged   | Auctioneer core health maintainer          | Measure cached verdict plus live preboot/boot probe bodies at concurrent terminal completion                                                    |
| `pre_admission_parser`   | unchanged   | EdgeZero adapter maintainer/provider   | Publish and negatively test provider ingress population, framing, parser and host-storage bounds at the memory scope                            |
| `body_streams`           | unchanged   | Auctioneer outbound/target maintainers     | Drive multiple waves with retained completed results, decoder windows and cancellation on the guest ABI                                         |
| `catalog_resident`       | unchanged   | Auctioneer catalog maintainer              | Measure real generated descriptors against the matched pre-catalog baseline without installation in the pure profile test                       |
| `catalog_initialization` | unchanged   | Auctioneer catalog maintainer              | Measure successful and failing catalog initialization separately from retained descriptors on the target ABI                                    |

### External Blockers

| Fact                                                | Owner                                | Next Verification                                                                                                              |
| --------------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `unknown_host_ingress_memory_accounting`            | EdgeZero adapter maintainer/provider | Publish and test host ingress/framing allocation ownership and bounds including cancelled and refused requests                 |
| `missing_proved_nonzero_separate_stack_requirement` | Auctioneer Fastly maintainer             | Prove a nonzero artifact/ABI/compiler-bound stack upper bound covering initialization, admitted depth, errors and cancellation |

## spin

Profile `spin_hosted`; dispatch `spin_generic`; layout `host_native_substitute`; reporter `aarch64-macos-64bit`; derived compiler target `wasm32-wasip2`.

Profile digest: `35d0efe1305669a64e71e2e623ae9d3e1457b268b33338b0982984271ec64edb`.

| Term                     |       Bytes | Allocation Owner                                                                     | Acquire                                                                     | Release                                                                                      | Overlap                                                                                                        |
| ------------------------ | ----------: | ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `baseline`               |    27328512 | [Auctioneer target/core maintainers](../../crates/auctioneer-core/src/bidder_registry.rs)    | Cold initialization of defaults, registry and singleton state               | Instance or execution teardown                                                               | Existing startup reserves remain additive with admitted work                                                   |
| `gvl`                    |     3072000 | [Auctioneer privacy maintainer](../../crates/auctioneer-privacy/src/gvl.rs)                  | GvlStore::get parses and caches a version; requests clone a handle          | Last cache or request handle, including requests pinning evicted versions                    | Cache entries plus distinct auction-held and auxiliary-held versions; handles are not duplicate parsed lists   |
| `auctions`               | 19588933144 | [Auctioneer core/exchange maintainers](../../crates/auctioneer-core/src/handlers/auction.rs) | Auction admission and all six phase receipts before config extraction       | Serialization completion or cancellation drops the work receipts                             | Config, decoded ingress, privacy projections, outbound requests and completed results can coexist              |
| `auction_egress`         |    50335744 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/handlers/auction.rs)   | Egress reservation before response serialization                            | Terminal response completion or cancellation, not work completion                            | Serialized body, provider handoff copies and transmit state can outlive auction work                           |
| `auxiliary`              |  3221233664 | [Auctioneer core auxiliary-route maintainer](../../crates/auctioneer-core/src/ingress.rs)    | Auxiliary route admission/work reservation or independent probe-config work | Route completion versus probe config resolution/render/publication; counters are independent | Auxiliary work and max(auxiliary, probe minimum) config work coexist with output handoff                       |
| `terminal_egress`        |        9216 | [Auctioneer core/target maintainers](../../crates/auctioneer-core/src/ingress.rs)            | Terminal/static output reservation, including detached head refusals        | Response completion or cancellation                                                          | Refusal output and transmit state are independent of auction and auxiliary work                                |
| `probe_output`           |       90112 | [Auctioneer core health maintainer](../../crates/auctioneer-core/src/ingress.rs)             | Independent probe-output reservation, including preboot responses           | Output completion; the cached verdict remains resident independently                         | Live probe output population plus one cached verdict, not probe-config work count                              |
| `pre_admission_parser`   |      238080 | [EdgeZero adapter maintainer/provider](../../crates/auctioneer-runtime/src/budget.rs)    | Host/framing/head parsing before Auctioneer route admission                     | Host/parser cleanup, including rejected and cancelled input                                  | No Auctioneer admission counter bounds the pre-admission population; outbound connections are not inbound requests |
| `body_streams`           |   698405888 | [Auctioneer outbound/target maintainers](../../crates/auctioneer-core/src/outbound/wave.rs)  | Global wave stream lease before dispatch and decode                         | Wave settlement, EOF or cancellation                                                         | Completed response slots survive stream release across waves and remain in auction response funding            |
| `catalog_resident`       |           0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental generated catalog/validator residency at initialization         | Instance or execution teardown                                                               | Zero incremental descriptor does not remove existing registry residency funded by baseline                     |
| `catalog_initialization` |           0 | [Auctioneer catalog maintainer](../../crates/auctioneer-core/src/bidder_registry.rs)         | Incremental catalog initialization scratch, including validation failure    | Initialization completion or error cleanup                                                   | Initialization remains an additive term even when no auction is currently executing                            |

Artifact proof: pending. Target ABI bound: pending. Known owner ABI bounds: 0/11. The charged population is not evidence of provider ingress enforcement.

### Components

These are explanatory, already-scaled ledger entries, not additional summands. Nested phase/subcomponent rows must not be added to their parents.

| Component            |       Bytes | Above 5% Of Demand |
| -------------------- | ----------: | ------------------ |
| `auction_config`     |   274790400 | no                 |
| `auction_ingress`    |   323740772 | no                 |
| `auction_privacy`    |    44040992 | no                 |
| `auction_projection` | 14890995188 | yes                |
| `auction_outbound`   |    17687552 | no                 |
| `auction_response`   |  4037678240 | yes                |
| `runtime_config`     |   274790400 | no                 |
| `request_side`       | 15214735960 | yes                |
| `call_side`          |  3943949312 | yes                |
| `bookkeeping`        |   101711872 | no                 |
| `debug`              |     9704608 | no                 |
| `assembly`           |    25165824 | no                 |
| `aux_config`         |   539373568 | no                 |
| `aux_body`           |     2424832 | no                 |
| `aux_query`          |      155648 | no                 |
| `aux_input_headers`  |       49152 | no                 |
| `aux_cookie`         |       98304 | no                 |
| `aux_signing`        |     1060864 | no                 |
| `aux_nodes`          |   833617920 | no                 |
| `aux_privacy`        |    21233664 | no                 |
| `aux_chooser`        |      794624 | no                 |
| `aux_urls`           |      196608 | no                 |
| `aux_retained_urls`  |      524288 | no                 |
| `aux_output`         |     1105920 | no                 |
| `aux_declared_work`  |  1610612736 | yes                |
| `aux_transmit`       |        8192 | no                 |
| `aux_handoff`        |  1610612736 | yes                |
| `aux_routes`         |  1610616832 | yes                |
| `probe_config`       |  1610616832 | yes                |

### Next Proofs

| Term                     | Disposition | Owner                                  | Required Proof                                                                                                                                  |
| ------------------------ | ----------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `baseline`               | unchanged   | Auctioneer target/core maintainers         | Measure cold and warm target residency, with initialization and failure paths identified separately                                             |
| `gvl`                    | unchanged   | Auctioneer privacy maintainer              | Prove target parsed-list bounds and eviction with multiple distinct request-pinned versions                                                     |
| `auctions`               | unchanged   | Auctioneer core/exchange maintainers       | Bound each phase on the target ABI at two sizes and unequal populations, including late failure and cancellation                                |
| `auction_egress`         | unchanged   | Auctioneer core/target maintainers         | Exercise a slow terminal handoff and cancellation while new auctions are admitted                                                               |
| `auxiliary`              | unchanged   | Auctioneer core auxiliary-route maintainer | Prove preboot/App-retained config defaults first, then cookie, signing, privacy, chooser, output and host-copy overlap on every auxiliary route |
| `terminal_egress`        | unchanged   | Auctioneer core/target maintainers         | Exercise preboot and saturated-class refusal output through actual target handoff                                                               |
| `probe_output`           | unchanged   | Auctioneer core health maintainer          | Measure cached verdict plus live preboot/boot probe bodies at concurrent terminal completion                                                    |
| `pre_admission_parser`   | unchanged   | EdgeZero adapter maintainer/provider   | Publish and negatively test provider ingress population, framing, parser and host-storage bounds at the memory scope                            |
| `body_streams`           | unchanged   | Auctioneer outbound/target maintainers     | Drive multiple waves with retained completed results, decoder windows and cancellation on the guest ABI                                         |
| `catalog_resident`       | unchanged   | Auctioneer catalog maintainer              | Measure real generated descriptors against the matched pre-catalog baseline without installation in the pure profile test                       |
| `catalog_initialization` | unchanged   | Auctioneer catalog maintainer              | Measure successful and failing catalog initialization separately from retained descriptors on the target ABI                                    |

### External Blockers

| Fact                                     | Owner                                | Next Verification                                                                                                              |
| ---------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `unknown_host_ingress_memory_accounting` | EdgeZero adapter maintainer/provider | Publish and test host ingress/framing allocation ownership and bounds including cancelled and refused requests                 |
| `platform_dispatch_mismatch`             | Auctioneer target maintainer             | Align actual dispatch metadata and proof with the adapter-selected deployment before treating its platform facts as applicable |

## Remediation Checkpoint

The audit freezes no projected savings. A concrete package must name exact changed owners, independent bounds, ordered deltas and residual gaps. AG must approve its immutable review commit before production ownership, admission or workload changes. Host facts, Fastly stack proof and the selected Spin dispatch remain independent prerequisites.

See [source investigation and rejected shortcuts](evidence/resource-envelope-audit-20261018.md) and the [implementation plan](../superpowers/plans/2026-10-17-m7-r0-wasm-resource-envelope.md).
