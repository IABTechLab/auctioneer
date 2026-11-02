# Auxiliary Response Handoff Audit

Date: 2026-10-18
Source baseline: `5344376b`; SDK `bc300ace5ca1ffe7060594a7bdb01c1da80de305`.
The separately approved resolver guard is under implementation at this audit.

This is a read-only ownership audit, not a new allocation measurement or
authorization to substitute a smaller handoff charge. The current source has
finite response-content bounds, but an all-route retained-storage bound has
not yet been established. Content length is not `Vec` capacity, header-map
storage or provider memory.

## Current Route Coverage

The registrations are in `edgezero.toml`; the owners below are actual request
and terminal-egress permits, not interchangeable concurrency declarations.

| Route                           | Body bound or current-source derivation                                                                                                               | Terminal owner                                    |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| Auction POST                    | Nongrowing capped success serializer; `final_response_bytes`. Fixed rendered errors fit 256 bytes.                                                    | Auction egress; static permit for rendered errors |
| Status GET                      | Fresh/cached/degraded output is sized before allocation and bounded by 8,192 bytes; empty healthy result is 204.                                      | Independent probe-output permit                   |
| Bidder list GET                 | Current-source maximum 99,073 content bytes; uncapped `serde_json::to_vec`. Invalid boolean query errors are fixed.                                   | Auxiliary permit and work receipt                 |
| Bidder detail/all GET           | Current compiled detail maximum 228; aggregate maximum 157,697 content bytes. Uncapped serialization; missing name returns empty 404.                 | Auxiliary permit and work receipt                 |
| GVL health GET                  | Unauthorized body is 12 bytes. Authorized output is below 512 bytes for the current archive/scope data, but serialization has no enforced output cap. | Auxiliary permit and work receipt                 |
| Cookie sync POST                | Append-before-extension body check at 262,144 bytes, including newline allowance.                                                                     | Auxiliary permit and work receipt                 |
| Cookie sync OPTIONS             | Empty 204/403 or fixed bounded error; accepted preflight drains its input separately.                                                                 | Auxiliary permit and work receipt                 |
| Set UID GET                     | Normal pixel/empty/warning and error bodies fit 256 bytes.                                                                                            | Auxiliary permit and work receipt                 |
| Get UIDs GET                    | Body limit 262,144, including HTML escaping/newline. Raw and escaped intermediates may coexist.                                                       | Auxiliary permit and work receipt                 |
| Unknown route/wrong method      | Fixed 404/405; bounded Allow value. Drain overflow/timeout use fixed 400/408 bodies.                                                                  | Auxiliary permit and work receipt                 |
| Pre-admission/admission failure | Static permit acquired before rendering; fixed bodies fit 256. If no static permit is available, abort instead.                                       | Static terminal permit                            |

Source owners: `handlers/{auction,status,info_bidders,health_gvl,cookie_sync,
sync_http}.rs`, `ingress.rs`, `host_primitives/static_egress.rs` and
`serialize.rs` in `auctioneer-core`. No automatic general HEAD/OPTIONS route is
assumed. Config-resolution failures are bounded separately by their closed
response vocabulary; the pending funding guard must use that same boundary.

The current info bounds use the effective 256-identity validation ceiling and
64-byte output-eligible names. Worst-case JSON escaping yields
`1 + 256 * (6*64 + 3) = 99,073` list bytes and
`1 + 256 * (6*64 + 4 + 228) = 157,697` aggregate bytes. The 228-byte detail
bound includes current optional Mocktioneer metadata, alias ownership and
the longer disabled status. These are current-source derivations, not universal
limits for the future M7 catalog. Metadata or identity-ceiling changes must
rederive them, not reuse these literals as allowances.

## Headers, Work And Populations

Sync decorators check at most 16 application-owned header fields and 16,384
serialized header bytes, excluding one separately bounded 4,096-byte complete
Set-Cookie value. These checks run after header construction. They are not a
capacity bound and do not count provider-added framing. CORS reflection is an
exact match to a validated configured origin, not arbitrary request text.

Even the conservative combination of 262,144 body bytes and 20,494 serialized
header bytes is only **282,638 content bytes**, not a physical-storage `R`.
Transient serializer trees, info catalog clones, raw/escaped buffers and cookie
formatting belong in work `W`; retained response backing and headers belong
in `R`. Avoid subtracting output allowances already included in `W` twice.

Preserve the distinct populations: auxiliary routes `A`, probe config work
`max(A,1)`, four independent probe-output permits, one cached status banner,
and static/auction terminal permits. Probe config work ends after
rendering/publication; its count cannot bound terminal probe outputs.

## Required Before Handoff Credit

1. Derive checked info/health maxima from typed caps and compiled metadata,
   and serialize with explicit nongrowing capacity. Bound sync final backing
   capacity as well as length, without narrowing currently accepted inputs.
2. Bound retained header-map storage and construction overlap, including the
   complete cookie formatter. Verify capacity and allocation controls at
   independent sizes, rejection/recovery, cancellation and terminal release.
3. Execute applicable guest/layout controls. Keep provider-added storage and
   unavailable provider facts separate; native requested-allocation tests are
   not hosted RSS or a provider copy contract.
4. Only then correct the runtime-owned handoff component with independently
   bounded `R`, retaining work and every independently gated population.

The existing handoff factors (Cloudflare 3, others 2) remain conservative
declarations, not new verified physical guarantees. No limit, coefficient,
equation, receipt or fit result changes as a consequence of this audit.
