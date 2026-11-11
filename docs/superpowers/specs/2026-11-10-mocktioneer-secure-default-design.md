# Make Mocktioneer available with secure defaults

The maintainer approved this change on November 10 for PR #160. It supersedes
the feature-gated publication and compiled-loopback fallback in the original
[endpoint design](2026-11-09-mocktioneer-runtime-endpoint-design.md).

Mocktioneer is compiled in normal builds and is disabled by default. Enabling
it requires an operator-configured endpoint; omitted/null settings can inherit
a base identity's endpoint but never an implicit destination. Both normal and
development builds require that explicit configuration. Public HTTPS works in
normal CLI and serving builds. The existing `insecure-bidders` feature permits
only the existing HTTP/exact-loopback development exception for configured
Mocktioneer endpoints. URL admission, request-level override rejection, safe
diagnostics and captured sink authority remain unchanged.

Compiled bidder metadata represents absence of an endpoint with `Option<String>`
instead of an invalid URL or a placeholder destination. Existing endpoint strings
retain their exact values. Build admission allows an absent endpoint only for a
factory with typed endpoint settings whose definition is disabled by default;
empty or otherwise invalid supplied defaults still fail. This prevents accidental
endpoint omission from silently changing other bidders' contracts. The endpoint
builder rejects missing templates, and the exchange cannot construct or send a
request when neither configuration nor metadata provides a destination.

Mocktioneer's validator checks every supplied endpoint even when disabled or
shadowed. It additionally rejects an enabled identity without a resolved endpoint,
using the static reason `required when enabled`. Direct endpoint selection also
rejects absence, independently of prior configuration validation. Alias settings
inherit through the existing requested/base resolver; activation and the
`implementation` selector keep their current independent roles.

Add explicit generated-App acceptance for the configured-endpoint publication
cohort while preserving the frozen 14 priority flows. Historical qualification
translation retains its original timeout/sample-rate bidder row independently.

Update owner-local tests, metadata/build admission controls, shared synthetic
fixtures and CLI qualification setup to select destinations explicitly. Keep
protocol captures, frozen allocation thresholds and workload limits unchanged.
Run both default and development lanes, public settings/endpoint contracts,
compiler/allocation probes, strict lint, provider compilation and actual local
Fastly guest checks. Preserve local configuration and do not contact a hosted
bidder or deploy as part of verification.
