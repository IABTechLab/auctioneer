# Operator Configuration Ownership Inventory

Date: 2026-11-02

Status: Draft supporting
[the proposed configuration contract](2026-11-02-operator-runtime-configuration-design.md).
Current-source facts are pinned to Auctioneer
`7d7fee7704e2d9552457068ac7de4861b6c7169e`; destinations are proposals, not
implemented readers. Issue: [#125](https://github.com/IABTechLab/auctioneer/issues/125).

## 1. Reading The Inventory

- `C`: compiled product code, declarations, reviewed defaults or safety facts.
- `S`: operator/host startup intent; validate before loading policy and freeze
  for the configuration domain. Defaults may be product-owned, not customer-owned.
- `P`: operator runtime policy; replace as a validated coherent snapshot.
- `V`: a secret value supplied by the host secret service, not ordinary config.
- `D`: derived/effective state; no independent public setter.

Paths in the tables name current Rust fields unless a current authoring path is
explicitly shown. They are not promises that those exact Rust structs can be
deserialized safely as future public input. Nested groups below enumerate their
leaves; `{a,b}` means both named fields, not an unspecified prose bucket.

For `P` rows, source/trust is the selected operator-policy source from the main
contract; validation is shared typed and effective semantic validation in core;
lifetime is one snapshot plus its admitted readers. Request-time copies remain
charged to auction, auxiliary or probe work. `S` rows use the pre-established
startup input bound, pure validation and the installed host/adapter profile;
startup and retained storage must be funded. `C`, `V` and `D` rows retain their
owning crate/host's reviewed admission and accounting rules. These are common
columns, not authorization to bypass a more specific rule below.

## 2. Authoring And Build Inputs

| Current source | Current behavior | Disposition |
| --- | --- | --- |
| `AuctioneerConfigSource.{baked.host,baked.account,runtime}` | All required. Build types the baked pair; CLI types runtime. | Reject `baked` in the new normal authoring path; strictly type `startup` and `runtime`. Build consumes neither operator subtree. |
| `HostDefaultRaw` | `{gdpr,ccpa,lmt,host_cookie,user_sync,status_response,resource_limits,aliases}` | Leaf dispositions are section 3. Retain only reviewed neutral defaults as `C`; operator values move to `P`. |
| `AccountRaw` | `{disabled,debug_allow,bid_rounding,targeting_prefix,truncate_target_attribute,gdpr,cookie_sync,ccpa,privacy}` | Leaf dispositions are section 4. Add a real `P` account-default scope; named accounts are not a substitute. |
| `runtime-bootstrap.toml` | Compiled `{runtime.{allow_missing_config,max_config_fetch_ms}}` | Replace with `S` source selection and extraction timeout. No operator-specific rebuild. |
| `bidder.toml`, `parameters.json`, provenance/witnesses | Build-owned discovery, typed settings vocabulary, schemas and publication gates | `C`; not moved to operator policy or a runtime compiler. |
| Disclosure sources, field classification, EEA data, GVL/GCL archive and lock | Reviewed/pinned code/data and archive admission | `C`; preserve source checks and archive refresh controls; not arbitrary operator policy. |
| `AUCTIONEER__DEFAULTS__PATH_OVERRIDE`, `AUCTIONEER__BIDDERS__DIR`, `AUCTIONEER__DISCLOSURE__DIR`, `AUCTIONEER__RUNTIME_BOOTSTRAP__PATH_OVERRIDE` | Test-harness source replacement controls | Test controls only; never release configuration. Remove/retarget obsolete default-generation probes without weakening the feature-export boundary. |
| `AUCTIONEER__GVL__ARCHIVE__{DIR,MAX_AGE_DAYS,BUNDLE_MAX_RAW_MIB,BUNDLE_MAX_COMPRESSED_MIB,SKIP_BUNDLE_CHECK}` | Archive input/validation controls, not application policy; skip-bundle use is harness-gated | Preserve the archive/build-tool owner and witness checks; no startup/hot-policy alias is invented. |

Sources: [root schema](../../../crates/auctioneer-config-schema/src/lib.rs),
[raw DTO/codegen](../../../crates/auctioneer-config-schema/src/codegen/mod.rs),
[build owner](../../../crates/auctioneer-config/build.rs).

## 3. Effective Host Fields

All current host leaves have setter-shaped runtime overrides. That does not
make them independent of compiled fallbacks or give them complete clear/reset
semantics. The new host scope must replace that fallback ownership, not merely
rename the current optional overlay.

| HostConfig path | Class / proposed source | Validation and resolution notes |
| --- | --- | --- |
| `gdpr.enabled`, `gdpr.default_value` | `P` host policy over `C` neutral defaults | Preserve existing master-switch/applicability behavior; default signal is exactly `"0"` or `"1"`. Load failure is not a signal value. |
| `gdpr.host_vendor_id` | `P` host policy; neutral undeclared product value | Preserve the reviewed `0`/undeclared handling and vendor-ID bounds. This changes one snapshot's policy, not a secret/authorization binding. |
| `gdpr.non_standard_publishers` | `P` host policy; product list empty | Whole-list replacement; no customer account IDs in the product artifact. |
| `gdpr.tcf2.enabled` | `P` host policy | Account/channel fallback stays owned by the current privacy resolver. |
| `gdpr.tcf2.purpose_one_treatment.{enabled,access_allowed}` | `P` host policy | Both host fields are booleans. Keep account inheritance separate. |
| `gdpr.tcf2.purposes.<id>.{enforce_algo,enforce_purpose,enforce_vendors,vendor_exceptions}` | `P` host policy | IDs `1..=10`; algorithm `full/basic`; tri-states `unset/enabled/disabled`; list replacement and existing publisher-restriction safeguards. |
| `gdpr.tcf2.special_features.<id>.{enforce,vendor_exceptions}` | `P` host policy | IDs `1..=12`; tri-state/list rules. No invented algorithm/transparency field. |
| `ccpa.enforce`, `lmt.enforce` | `P` host policy | Both currently have real runtime setters. Preserve independent privacy composition. |
| `host_cookie.{cookie_name,family,domain}` | `P` host policy | Product cookie name/family may be generic protocol defaults; product domain is absent. Cookie syntax/size and signed-cookie behavior remain enforced. |
| `host_cookie.{ttl_days,max_cookie_size_bytes}` | `P` host policy | Preserve permitted ranges and independent complete-cookie ceiling; zero follows the field's existing meaning, not a generic unlimited rule. |
| `host_cookie.optout_cookie.{name,value}` | `P` optional policy | Optional configured opt-out cookie; no customer value in product defaults. Preserve denial-only semantics. |
| `user_sync.external_url`, `user_sync.redirect_url` | `P` explicit service configuration | No example/customer URL is a configured product default. Proposed unconfigured service state is explicit; configured URLs retain current safety/template validation. |
| `user_sync.cooperative.enabled_by_default`, `user_sync.priority_groups`, `user_sync.allowed_origins` | `P` service policy | Ordered/whole-list rules, family/name limits and credentialed CORS remain. Product origin and priority lists are empty. |
| `status_response` | `P` host policy over a generic product banner | Existing bounded banner; degraded liveness never echoes arbitrary failed config. Current overlay spelling is `status.banner`. |
| `aliases.<alias>` | `P` host policy; product operator-alias map empty | Namespace/case/cycle/base/catalog checks. Distinct from immutable bidder metadata `aliasOf`. |
| `resource_limits.{default_tmax_ms,max_tmax_ms}` | `P` requested time limits | Preserve range/cross-field constraints and absolute deadline ownership. |
| `resource_limits.{max_bidder_response_bytes,max_bidders_per_auction,max_outbound_requests}` | `P` requested caps | Current overlay is `auction.*`; resolve each independently against funded `S` limits and keep warnings attributed to the observable bidder. |
| `resource_limits.{max_outbound_raw_bytes,max_outbound_request_bytes,max_transport_bytes,max_decoded_bytes,max_parsed_nodes}` | `P` requested caps | Raw/encoded, transport/decoded and syntax-node limits are distinct; they can narrow, never resize installed capacity. |

Sources: [effective fields](../../../crates/auctioneer-config-schema/src/types.rs),
[cookie/sync fields and validators](../../../crates/auctioneer-config-schema/src/usersync.rs),
[current runtime setters](../../../crates/auctioneer-core/src/config/mod.rs),
[host resolution](../../../crates/auctioneer-core/src/config/resolve.rs).

## 4. Account Defaults And Named Accounts

These leaves all become `P` in `account_defaults` and in named `accounts`.
Today only targeting prefix and truncation have a global runtime overlay;
the other default-account leaves remain compiled even though named accounts
can override them. Product defaults cannot be a customer-specific account.

| AccountConfig path | Validation / merge owner |
| --- | --- |
| `disabled`, `debug_allow` | Shared effective validation and current route-specific disabled/debug authorization; explicit false remains a value. |
| `bid_rounding`, `targeting_prefix`, `truncate_target_attribute` | Existing rounding, length/range and targeting behavior; optional truncation needs explicit reset semantics. No second legacy global-targeting precedence layer. |
| `gdpr.enabled` | Tri-state account enablement; preserve host/channel fallback and explicit account opt-in. |
| `gdpr.channel_enabled.{web,app,amp,video,dooh}` | Five independently resolved tri-states; CCPA and GDPR retain their distinct owners. |
| `gdpr.purposes.<id>.{enforce_algo,enforce_purpose,enforce_vendors,vendor_exceptions}` | Same purpose vocabulary and ID bounds as host; preserve account-to-host enforcement fallback and Basic safeguards. |
| `gdpr.special_features.<id>.{enforce,vendor_exceptions}` | Same special-feature vocabulary and bounds; keyed merge does not concatenate exception lists. |
| `gdpr.basic_enforcement_vendors` | Account-only whole-list replacement; no new host field or central bidder-name list. |
| `gdpr.purpose_one_treatment.{enabled,access_allowed}` | Optional booleans; absence/inherit and explicit clearing must not collapse into a grant. |
| `cookie_sync.{default_limit,max_limit,default_coop_sync,priority_groups}` | All effective fields optional; preserve limit/cooperation/host-group fallback and empty-list distinctions. |
| `ccpa.enabled`, `ccpa.channel_enabled.{web,app,amp,video,dooh}` | Existing tri-state/channel defaults and malformed-selected-signal enforcement. |
| `privacy.ipv4.anon_keep_bits`, `privacy.ipv6.anon_keep_bits` | Separate `0..=32` and `0..=128` bounds. |
| `privacy.allowactivities.<activity>.{default,rules}` | Merge by activity key and independently supplied default/rules fields. Supplied rules replace the complete array, including []; omitted fields inherit. Parent default false survives child rules []. |
| Each activity rule: `allow`, `condition.componentName`, `condition.componentType` | Ordered matching, rule/condition/name bounds; no rejected values in validator parameters. |

Activity vocabulary is `fetchBids`, `transmitUfpd`, `transmitPreciseGeo`,
`transmitTid`, `syncUser`, `reportAnalytics`, `enrichUfpd`,
`transmitUniqueRequestIds`. Component types are `bidder`, `analytics`, `rtd`,
`general`. These names are product vocabulary, not configurable enum additions.
Current limits are 128 rules/activity, 128 combined condition entries/rule,
and 128 UTF-8 bytes/component name; this design does not alter them.

`AccountOverride.max_transport_bytes` and `max_decoded_bytes` are also `P` for
both deployment account defaults and named accounts, even though they are not
fields of effective `AccountConfig`. The shared response-budget resolver owns
their additional precedence layer. Account identity selection/source remains
derived request state, not a configurable replacement for a publisher ID.

Sources: [effective account fields](../../../crates/auctioneer-config-schema/src/types.rs),
[activity model](../../../crates/auctioneer-config-schema/src/activity.rs),
[current account overlays](../../../crates/auctioneer-core/src/config/mod.rs),
[selection/merge](../../../crates/auctioneer-core/src/config/resolve.rs),
[budget precedence](../../../crates/auctioneer-core/src/config/budgets.rs).

## 5. Other Runtime Fields And Bidder Metadata

| Field group | Class and owner |
| --- | --- |
| Proposed `runtime.schema_version` | `C` supported application format, carried as required constant `1` in the new payload and checked separately from SDK envelope version. Not an operator-selected compatibility mode. |
| `features.{gzip_outbound,returnallbidstatus_default}` | `P`; preserve typed feature behavior. A runtime flag does not create a missing compiled feature. |
| `floor_currency.{expires_at,usd_per_unit.<currency>}` | `P`; finite/positive rates, currency codes and freshness stay validated by their current owner; no external feed added. |
| `bidders.<identity>.{settings,max_transport_bytes,max_decoded_bytes,enabled,usersync_enabled,timeout_ms,sample_rate}` | `P`; generated settings value types, catalog/alias eligibility, per-field fallback, range/sample validation and implemented readers. Admission is not inferred from a field merely being declared. |
| Per-owner setting names/types and endpoint macro vocabulary | `C`, generated from the owning `bidder.toml`; values admitted through `P` and existing bounded secret references. |
| Manifest `{key,factory,required_feature,native_only,endpoint.fields,settings.fields,definitions,provenance}` | `C`; source discovery and target/feature eligibility, not policy setters. |
| Metadata identity, implementation and alias ancestry | `C`: definition key/name, implementation binding, `aliasOf`, `required_feature`, `native_only`. |
| Metadata `{defaultEnabled,disabled,debug.allow,bidderTimeoutMs}` | `C` release defaults/restrictions. Runtime can use supported activation/timeout/debug rules, never override an immutable disable or manufacture eligibility. |
| Metadata `{endpoint,hostAllowlist,supported_endpoint_macros,endpointCompression}` | `C` protocol/routing safety. Approved bidder-owned runtime routing settings still pass the bounded endpoint builder; no universal unrestricted URL override. |
| Metadata `{gvlVendorID,maintainer.email,capabilities.{site,app,dooh}.mediaTypes}` | `C` reviewed identity/capabilities/attribution, not operator privacy decisions. |
| Metadata `userSync.{key,iframe.{url,userMacro},redirect.{url,userMacro},supports,enabled,skipWhen.{gdpr,gpp_sid},defaultResponse}` | `C` sync-family/protocol declaration, distinct from operator service URL and activation policy. |
| Metadata `openrtb.{version,gpp-supported}` | `C` advertised protocol support; a config flag cannot upgrade the implementation. |
| Provenance repository/revision/tree/blob/path/byte/hash/retained/transformation records and metadata heap descriptors | `C` / `D`; preserve attribution and witness checks; no operator can supply their own evidence of code eligibility. |

Sources: [current runtime DTOs](../../../crates/auctioneer-core/src/config/mod.rs),
[currency](../../../crates/auctioneer-config/src/currency.rs),
[metadata raw DTO](../../../crates/auctioneer-config-schema/src/codegen/mod.rs),
[manifest schema](../../../crates/auctioneer-config-schema/src/bidder_manifest.rs),
[discovery](../../../crates/auctioneer-config-schema/src/bidder_discovery.rs).

## 6. Startup, Resource Intent And Actual Facts

Current `ResolvedIsolateLimits` fields are source-owned adapter declarations;
they are not currently a public runtime input document. Their proposed
startup exposure must preserve a real installed enforcement point. A field
that only charges an allowance is not advertised as an active workload knob.

| Current field(s) | Class / proposed rule |
| --- | --- |
| `RuntimeBootstrap.allow_missing_config` | `S`; replace the boolean fallback with explicit source selection. Required store/host failures never select defaults mode. |
| `RuntimeBootstrap.max_config_fetch_ms` | `S`; extraction-wide timeout installed before reads, including the SDK secret walk. |
| Config logical binding/key, approved physical binding and source selection | `S`; provider/host registry, not runtime data. Provider-specific key constraints must be validated rather than assumed portable. |
| ConfigExtractionLimits `{max_backend_bytes,max_blob_bytes,max_json_depth,max_json_nodes,max_secret_bytes,max_total_bytes,timeout}` | Exact pinned SDK fields. `S` only with an explicit installed enforcement/accounting mapping; otherwise fixed `C` settings. The existing row directly funds backend/blob/value caps, not automatic new secret/depth/total knobs. JSON nodes here count values, not syntactic keys. Distinct from the raw startup-file read bound. |
| `isolate_total_bytes` | `S` configured ceiling; preserve independent known platform ceiling/unknown facts and headroom rules. |
| `baseline_bytes` | `C` / `D` measured baseline plus documented reserves. Only separately identified additional reserve may be requested; never accept a smaller claimed actual baseline. |
| `inbound_request_bytes`, `max_outbound_request_bytes`, `max_outbound_raw_bytes`, `max_transport_bytes`, `max_decoded_bytes`, `final_response_bytes` | `S` supported byte-cap intent; enforce the corresponding actual read/construction/serialization boundary and existing floors. |
| `inbound_parse_factor`, `response_parse_factor`, `response_node_peak_bytes` | `S` conservative reservations with `C` independently established minimums. No request can lower the cost below actual layouts. |
| `egress_permits`, `aux_route_permits`, `max_concurrent_auctions`, `max_active_body_streams`, `terminal_response_permits` | `S` installed gate capacities, immutable while requests hold permits. No per-App private replacement gates. |
| `response_write_deadline_ms`, `max_brotli_wbits`, `max_brotli_layers` | `S` supported deadline/decoder intent; preserve decoder caps, validation and actual enforcement. |
| `aux_route_peak_bytes` | `S` reservation, at least the complete derived floor; not permission to invent a smaller actual aux cost. |
| `guest_decoder_bound_bytes`, `refusal_body_bytes`, `transmit_state_bytes` | `C` / `D` actual SDK/owning-layout costs or bounds; no arbitrary operator setter. |
| `gvl_cache_entries`, `gvl_parsed_bytes` | `C` / `D` actual cache population and reviewed parsed-list bound. A different declared population does not resize today's cache; any future public cache knob must change its real owner and remain funded. |
| `max_bidders_per_auction`, `max_outbound_requests`, `max_parsed_nodes`, `max_impressions_per_auction` | `S` funded population ceilings; `P` may request only supported narrowing where a policy reader exists. |
| `max_runtime_config_bytes`, `max_runtime_config_blob_bytes`, `max_config_json_nodes` | `S` extraction caps, independently normalized/installed/funded. Config JSON values are not syntax nodes. |
| `max_copied_header_bytes`, `max_admitted_header_bytes`, `max_admitted_header_count` | `S` supported enforcement intent; adapter/SDK parser congruence and independent raw-ingress charges remain necessary. |
| `TargetPlatformConstants.platform_concurrent_ceiling` | `C` actual pre-admission platform population; not Auctioneer admission intent. |
| Target header/parser fields `{header_entry_overhead_bytes,parser_header_ceiling_bytes,parser_header_knob_bytes,max_request_target_bytes,h1_max_buf_size_bytes,h1_max_headers,parser_max_header_entries,pseudoheader_allowance_bytes,parser_transient_bytes,egress_handoff_factor}` | `C` / `D` adapter/SDK facts and derived installed knobs. Normalize supported startup intent through the adapter; policy cannot supply a cheaper fact. |
| `ResourceProfile.{name,dispatch_name,target_abi,headroom_class,memory_scope,platform,dispatch_platform}` | `C` selected adapter/deployment identity and facts. Labels are not proof of equivalence. |
| Profile `{limits,target,outbound_meta_bytes,construction_costs,catalog_costs,log_sink}` and `BootEnvelope` | `S` admitted intent + `C`/`D` owning costs/facts/logging; preserve those separate owners in the assembled input. |
| `RequestConstructionCosts.{fixed_per_auction_bytes,reusable_scratch_bytes,retained_per_scope_bytes}` | `C` / `D` owner-derived layouts; no user-supplied replacement costs. |
| `CatalogResourceCosts.{resident_bytes,initialization_bytes,schema_validation_per_auction_bytes,context_per_auction_bytes}` | `C` / `D` existing catalog/schema cost owner, not operator-policy bytes. |
| Boot normalized limits, totals/charges/margins, host services, resolved policy/account/alias/budget objects | `D`; opaque validated construction, no public deserialization shortcut. |

Zero currently normalizes the three config caps to SDK defaults. The new
startup validator must distinguish omission/defaulting from an invalid explicit
value; it must not import that internal sentinel into a public unlimited mode.
The requested/default/normalized triple must remain visible to diagnostics.

Sources: [limit and target types](../../../crates/auctioneer-runtime/src/budget.rs),
[profile](../../../crates/auctioneer-core/src/host_primitives/resource_profile.rs),
[normalization/install](../../../crates/auctioneer-core/src/host_primitives/gates.rs),
[captured SDK caps](../../../crates/auctioneer-core/src/configure.rs).

## 7. Adapter, CLI And Secret Surfaces

| Current surface | Class / cutover rule |
| --- | --- |
| Axum host/port, raw ingress/parser and file-store snapshot settings | `S` SDK-owned deployment inputs; preserve the existing native preflight and actual bounded snapshot reads. SDK variables retain `EDGEZERO__...` names. |
| Cloudflare Worker vars/secrets and KV binding names | `S` provider translation and `V` secrets; use declared binding names, not a fictional process environment or enumerable binding map. |
| Fastly linked config/secret stores and logging hook | `S` provider resource links/logical aliases and `V` secrets. Key/location restrictions must match the serving SDK path; a generic selector that only works on Axum is not sufficient. |
| Hosted Spin variables/KV labels and logging level | `S` hosted-provider translation and `V` secrets; distinguish hosted deployment facts from generic Spin local execution. |
| `AUCTIONEER__RUNTIME__ALLOW_UNAUTHENTICATED_HEALTH` / provider mapping | `S` health-only authorization intent, distinct from fatal domain settings. Malformed input never opens health or logs its raw value. Preserve Axum-fatal versus edge-Closed resolution; installation conflicts remain fatal. |
| `AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN` / provider secret mapping | `V`; current platform reads directly materialize values outside the runtime extractor. #128/#130 and provider ports must establish accepted-size, materialization and applicable time bounds. Core does not read provider environment values. Current health policy is frozen, not freely rotating per request. |
| `usersync_cookie_keyring` | `V`, one existing bounded secret read at the route's UID stage; preserve active/previous key rotation and deadline/cancellation behavior. |
| Bidder credential references and coherent bundles | Permitted typed references through `P` within startup-approved bindings; secret values `V` through the existing post-privacy/schema bounded reader. No raw secrets in output policy. |
| SDK logging settings and host service implementations | `S`; establish the logging sink before diagnostics and keep startup materialization/residency accounted. |
| CLI config push/diff selector fields `{adapter,app_config,manifest,store,key,local,runtime_config,staging}` | Invocation/deployment intent, not policy leaves. SDK `runtime_config` chooses Spin backend configuration; it is not the Auctioneer `runtime` subtree. |
| CLI switches `{dry_run,no_diff,no_env,yes,format,exit_code,strict}` | Invocation controls, not deployed policy or persistent bypasses. Keep the applicable command's actual options; not all commands own all switches. |
| CLI deploy `{adapter,adapter_args,application_release,service_id,staging}` | Deployment invocation, external authorization and release identity; no customer data enters executable generation. |

Current app-source environment overlay changes only scalar leaves already
present in the authoring input. `--no-env` disables that overlay, not all SDK
store-selector environment resolution. The new CLI must state the distinction
and validate the final source/target combination; it cannot claim full environment
isolation just because the policy overlay was disabled.

Sources: [CLI application](../../../crates/auctioneer-cli/src/application.rs),
[typed push source](../../../crates/auctioneer-cli/src/config_source.rs),
[bounded cookie-key reader](../../../crates/auctioneer-core/src/handlers/usersync_keys.rs),
[health policy](../../../crates/auctioneer-core/src/host_primitives/registry.rs),
[logical declarations](../../../edgezero.toml), four adapter entrypoints and
their provider manifests.

## 8. Known Gaps, Not Granted Capabilities

- The complete default-account policy is not currently hot-configurable.
- Runtime overlay validation still references compiled operator fallbacks.
- Public Rust `Default` is not a substitute for reviewed product defaults:
  current effective resource defaults include zeros, and UserSyncConfig's
  default URL is an example endpoint, not an operator configuration.
- Source-controlled boot rows do not mean arbitrary startup resource settings
  already have actual readers, bounds or funded ownership.
- A bounded runtime extractor does not bound a preceding TOML file read,
  environment/binding materialization or host-settings parser.
- Typed Trusted Server settings are not evidence of bounded ingestion. Its
  current SDK revision and services differ from Auctioneer's; align the actual
  selected integration contract before claiming type/behavior equivalence.
- An unkeyed envelope digest is not authentication, freshness, replay
  protection or an application-schema compatibility negotiation.
- Successful boot, a metadata row, a native test, or a shared code hash is not
  guest eligibility, platform fit, live authorization or publisher acceptance.
