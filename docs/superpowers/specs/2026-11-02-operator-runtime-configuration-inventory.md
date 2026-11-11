# Operator Configuration Ownership Inventory

Date: 2026-11-02

Status: Field census with active dispositions aligned on 2026-11-04 to
[the single-runtime contract](2026-11-02-operator-runtime-configuration-design.md).
Historical source facts remain pinned to Auctioneer
`7d7fee7704e2d9552457068ac7de4861b6c7169e`; retired names below identify migration
inputs, not accepted interfaces or implementation completion. Issue:
[#125](https://github.com/IABTechLab/auctioneer/issues/125).

## 1. Reading The Inventory

- `C`: compiled product code, declarations, reviewed defaults or safety facts.
- `S`: existing host/adapter deployment inputs, bindings and installed limits;
  not an Auctioneer startup DTO, operator payload or new configuration mode.
- `P`: operator runtime policy; replace as a validated coherent snapshot.
- `V`: a secret value supplied by the host secret service, not ordinary config.
- `D`: derived/effective state; no independent public setter.

Paths in the tables name baseline Rust fields unless an active authoring path is
explicitly shown. They are not promises that those exact Rust structs can be
deserialized safely as future public input. Nested groups below enumerate their
leaves; `{a,b}` means both named fields, not an unspecified prose bucket.

For `P` rows, one direct `OperatorPolicy` supplies root `schema_version = 1`,
`host`, `account_defaults`, `accounts`, `bidders`, `features` and `floor_currency`.
Existing EdgeZero typed validate/diff/push and bounded SDK extraction reuse shared
semantic validation. Typed-policy helpers accept caller-owned input/context;
actual host ingestion/adoption and capacity/lifetime proof remain #135.
Request-time copies stay charged to auction, auxiliary or probe work. `S` rows
retain existing host/adapter validation, funding and installation, without a
new startup package or numeric reservation. `C`, `V` and `D` rows retain their
owning crate/host's reviewed admission and accounting rules. These are common
columns, not authorization to bypass a more specific rule below.

## 2. Authoring And Build Inputs

| Historical source | Baseline behavior | Active disposition |
| --- | --- | --- |
| Retired `AuctioneerConfigSource.{baked.host,baked.account,runtime}` and staged `OperatorConfigSource` | Build/CLI split or startup/runtime root | Direct `OperatorPolicy` only; reject baked/startup/runtime wrappers and backward wire aliases. The SDK wrapper adds no layout or projection. |
| `HostDefaultRaw` | `{gdpr,ccpa,lmt,host_cookie,user_sync,status_response,resource_limits,aliases}` | Leaf dispositions are section 3. Retain only reviewed neutral defaults as `C`; operator values move to `P`. |
| `AccountRaw` | `{disabled,debug_allow,bid_rounding,targeting_prefix,truncate_target_attribute,gdpr,cookie_sync,ccpa,privacy}` | Leaf dispositions are section 4. Add a real `P` account-default scope; named accounts are not a substitute. |
| Retired `runtime-bootstrap.toml` | Compiled missing-policy fallback and extraction timeout | No startup file, source modes or default-source fallback. Required policy absence refuses; extraction timeout remains host-owned. |
| `bidder.toml`, `parameters.json`, provenance/witnesses | Build-owned discovery, typed settings vocabulary, schemas and publication gates | `C`; not moved to operator policy or a runtime compiler. |
| Disclosure sources, field classification, EEA data, GVL/GCL archive and lock | Reviewed/pinned code/data and archive admission | `C`; preserve source checks and archive refresh controls; not arbitrary operator policy. |
| Product-default, bidder and disclosure test-harness overrides; retired bootstrap-file override | Test-harness source replacement controls | Test controls only; never release configuration. Retire bootstrap-file probes and preserve neutral product-data/feature-export rejection controls. |
| `AUCTIONEER__GVL__ARCHIVE__{DIR,MAX_AGE_DAYS,BUNDLE_MAX_RAW_MIB,BUNDLE_MAX_COMPRESSED_MIB,SKIP_BUNDLE_CHECK}` | Archive input/validation controls, not application policy; skip-bundle use is harness-gated | Preserve the archive/build-tool owner and witness checks; no startup/hot-policy alias is invented. |

Sources: [policy model](../../../crates/auctioneer-config/src/policy.rs),
[raw product DTO/codegen](../../../crates/auctioneer-config/src/codegen/mod.rs),
[build owner](../../../crates/auctioneer-config-data/build.rs).

## 3. Effective Host Fields

The baseline host leaves map to `host` policy over neutral compiled defaults.
Use the contract's explicit clear/reset rules; declaring an optional overlay
alone is not semantic or resource admission.

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
| `status_response` | `P` host policy over a generic product banner | Active path `host.status_response`; bounded banner, with no arbitrary failed config in degraded liveness. |
| `aliases.<alias>` | `P` host policy; product operator-alias map empty | Namespace/case/cycle/base/catalog checks. Distinct from immutable bidder metadata `aliasOf`. |
| `resource_limits.{default_tmax_ms,max_tmax_ms}` | `P` requested time limits | Preserve range/cross-field constraints and absolute deadline ownership. |
| `resource_limits.{max_bidder_response_bytes,max_bidders_per_auction,max_outbound_requests}` | `P` requested caps | Active path `host.resource_limits.*`; resolve independently against funded host-owned limits and attribute warnings to the observable bidder. |
| `resource_limits.{max_outbound_raw_bytes,max_outbound_request_bytes,max_transport_bytes,max_decoded_bytes,max_parsed_nodes}` | `P` requested caps | Raw/encoded, transport/decoded and syntax-node limits are distinct; they can narrow, never resize installed capacity. |

Sources: [effective fields](../../../crates/auctioneer-config/src/types.rs),
[cookie/sync fields and validators](../../../crates/auctioneer-config/src/usersync.rs),
[current runtime setters](../../../crates/auctioneer-core/src/config/mod.rs),
[host resolution](../../../crates/auctioneer-core/src/config/resolve.rs).

## 4. Account Defaults And Named Accounts

These leaves are `P` in `account_defaults` and named `accounts`, replacing the
baseline's incomplete global overlay. Product defaults cannot identify a customer.

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

`AccountPolicy.max_transport_bytes` and `max_decoded_bytes` are also `P` for
both deployment account defaults and named accounts, even though they are not
fields of effective `AccountConfig`. The shared response-budget resolver owns
their additional precedence layer. Account identity selection/source remains
derived request state, not a configurable replacement for a publisher ID.

Sources: [effective account fields](../../../crates/auctioneer-config/src/types.rs),
[activity model](../../../crates/auctioneer-config/src/activity.rs),
[current account overlays](../../../crates/auctioneer-core/src/config/mod.rs),
[selection/merge](../../../crates/auctioneer-core/src/config/resolve.rs),
[budget precedence](../../../crates/auctioneer-core/src/config/budgets.rs).

## 5. Other Runtime Fields And Bidder Metadata

| Field group | Class and owner |
| --- | --- |
| Root `schema_version` | Required constant `1` in the direct policy, checked separately from SDK envelope version. No missing default or operator-selected compatibility mode. |
| `features.{gzip_outbound,returnallbidstatus_default}` | `P`; preserve typed feature behavior. A runtime flag does not create a missing compiled feature. |
| `floor_currency.{expires_at,usd_per_unit.<currency>}` | `P`; finite/positive rates, currency codes and freshness stay validated by their current owner; no external feed added. |
| `bidders.<identity>.{settings,max_transport_bytes,max_decoded_bytes,enabled,usersync_enabled,timeout_ms,sample_rate}` | `P`; generated settings value types, catalog/alias eligibility, per-field fallback, range/sample validation and implemented readers. Admission is not inferred from a field merely being declared. |
| Per-owner setting names/types and endpoint macro vocabulary | `C`, generated from the owning `bidder.toml`; values admitted through `P` and existing bounded secret references. |
| Bidder-owned configuration validation | Proposed `C` owner-local borrowed `validator::Validate` view, reached by existing discovery through one registration adapter; not operator-supplied rules, a second validation mechanism or an existing callback. See design section 5.1. All policy sources use the same dispatcher before admission. |
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
[metadata raw DTO](../../../crates/auctioneer-config/src/codegen/mod.rs),
[manifest schema](../../../crates/auctioneer-config/src/bidder_manifest.rs),
[discovery](../../../crates/auctioneer-config/src/bidder_discovery.rs).

## 6. Host-Owned Limits And Actual Facts

`ResolvedIsolateLimits` stays host/adapter-owned, not public policy or a new
startup input model. `P` can request only supported existing caps within those
installed limits. A charge-only allowance is not an active workload knob.

| Current field(s) | Class / proposed rule |
| --- | --- |
| Retired `RuntimeBootstrap.allow_missing_config` | No policy fallback/source mode; required policy absence refuses. |
| Retired `RuntimeBootstrap.max_config_fetch_ms` | Existing host-owned extraction deadline, including the SDK secret walk; no startup model/file. |
| Config logical binding/key and approved physical/trust binding | `S`; existing provider/host registry, never runtime policy. Keep provider-specific key constraints. |
| ConfigExtractionLimits `{max_backend_bytes,max_blob_bytes,max_json_depth,max_json_nodes,max_secret_bytes,max_total_bytes,timeout}` | Complete captured SDK tuple, with existing host enforcement/accounting. No new policy knobs. Backend/blob/value caps remain independent; JSON values are not syntactic keys. |
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

Zero internally normalizes the three config caps to SDK defaults. Host-owned
validation keeps omission/defaulting distinct from invalid explicit input;
the internal sentinel is not a public unlimited mode or new policy setter.
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
| `host.gvl.{authenticate_health,authenticate_health_token_secret}` | `P` health-only authorization. Omitted section or mode stays closed; true requires a nonempty valid default-store secret entry reference. False explicitly permits anonymous access only with the reference omitted; empty or named references alongside false are invalid. Anonymous health never reads a secret. A reference without a mode is invalid. Resolve through existing funded runtime config on each health request. No adapter policy flag remains. |
| Configured GVL health secret entry | `V` via the request's existing default Secret Store binding, maximum accepted payload 4096 bytes and existing config-fetch deadline. Fresh lookup per health request; empty/missing/faulted/excessive secrets deny access. Providers may materialize the complete value before the size check; synchronous provider calls have cooperative deadline checks. Axum uses SDK `EDGEZERO__SECRETS__<UPPERCASE_ENTRY>` with ASCII alphanumeric/underscore entry names; Cloudflare uses the literal entry name; Fastly uses the captured linked default alias; Spin lowercases the entry and needs declaration plus component mapping. The local Fastly `AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN` is provisioning only. Config visibility follows each provider's lifecycle, including Axum's startup file snapshot; secrets rotate through existing provider visibility. |
| `usersync_cookie_keyring` | `V`, one existing bounded secret read at the route's UID stage; preserve active/previous key rotation and deadline/cancellation behavior. |
| Bidder credential references and coherent bundles | Permitted typed references through `P` within host-approved bindings; secret values `V` through the existing post-privacy/schema bounded reader. No raw secrets in output policy. |
| SDK logging settings and host service implementations | `S`; establish the logging sink before diagnostics and keep startup materialization/residency accounted. |
| CLI config push/diff selector fields `{adapter,app_config,manifest,store,key,local,runtime_config,staging}` | Existing SDK invocation/deployment intent, not policy leaves or another Auctioneer config. SDK `runtime_config` chooses Spin backend configuration, not a payload subtree. |
| CLI switches `{dry_run,no_diff,no_env,yes,format,exit_code,strict}` | Invocation controls, not deployed policy or persistent bypasses. Keep the applicable command's actual options; not all commands own all switches. |
| CLI deploy `{adapter,adapter_args,application_release,service_id,staging}` | Deployment invocation, external authorization and release identity; no customer data enters executable generation. |

Current app-source environment overlay changes only scalar leaves already
present in the authoring input. `--no-env` disables that overlay, not all SDK
store-selector environment resolution. The new CLI must state the distinction
and validate the final source/target combination; it cannot claim full environment
isolation just because the policy overlay was disabled.

Sources: [CLI application](../../../crates/auctioneer-cli/src/application.rs),
[zero-layout SDK trait boundary](../../../crates/auctioneer-core/src/config/sdk.rs),
[bounded cookie-key reader](../../../crates/auctioneer-core/src/handlers/usersync_keys.rs),
[health policy](../../../crates/auctioneer-core/src/handlers/health_gvl.rs),
[logical declarations](../../../edgezero.toml), four adapter entrypoints and
their provider manifests.

## 8. Historical Baseline Gaps, Not Granted Capabilities

The following describes the pinned source census, not current progress. The
active destinations above supersede its operator-fallback/startup proposals;
no historical capture or evidence is rewritten to claim cutover acceptance.

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

## 9. Active Documentation And Script Surfaces

The 2026-11-04 reference census separates active instructions from historical
source records. This is a migration inventory, not runtime acceptance evidence.

| Surface | Active contract / retained boundary |
| --- | --- |
| `README.md`, `docs/architecture.md` | Direct versioned policy, `host`/`account_defaults`, neutral product data, no compiled operator fallbacks or startup source modes. |
| `docs/bidders/controlled-native.md`, bidder-registry design section 8 | Root version and `bidders` examples; aliases and immutable catalog vetoes stay unchanged. |
| Privacy-enforcement design section 7.3; user-sync design section 4.1 | `host`, `account_defaults` and named-account paths; independent privacy, inheritance and clear semantics. |
| Diagnostics/qualification design section 7 | Same typed-policy semantics, strict null/clear rules, no missing-policy fallback; retain SDK/request error and side-effect ordering. |
| Configuration-model ownership spec/plan | No startup/source DTO or new host normalization package; actual host ingestion/adoption proof is #135. |
| `docs/testing/provider-handlers.md` | Direct versioned synthetic account/debug policy, without mutating frozen provider captures. |
| `docs/testing/usersync-browser.md`, `scripts/check-usersync-browser.mjs`, `scripts/tests/usersync-browser-shutdown.test.mjs` | Generated envelope `data.schema_version = 1` and `data.host.user_sync`; keep JavaScript, real subprocess cleanup and existing credential isolation. |
| `scripts/raw-json-fixtures.allow` | Active product-default snapshot path retains its exact raw-fixture budget; no old snapshot-target alias. Workflow selector edits stay with their owner. |
| Main server design and older implementation/audit records | Authoring/DTO/bootstrap excerpts are historical, explicitly superseded by the current operator contract; they do not define supported wire aliases. |

Pinned `docs/testing/evidence`, frozen qualification/provider fixtures and external
privacy/reference captures remain byte-preserved. Compiled bidder/reference
metadata still legitimately uses "baked" terminology; it is not operator config.
