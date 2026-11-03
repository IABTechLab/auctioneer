# User Sync Cookie Integrity Design

Date: 2026-10-13
Status: Approved by AG and implemented; local evidence in the user sync verification record
Owner: user sync

## Scope And Trust

Authenticate Auctioneer-issued `uids` cookie state before any production UID
read. This is a hard wire cutoff: unsigned cookies, unknown versions,
unknown/retired keys and invalid MACs are rejected, not decoded as fresh
permissive state. No compatibility flag or unsigned fallback is added.

A valid MAC proves that an issuer holding the configured key wrote the
payload. It does not prove ownership of a bidder UID. `/setuid` remains a
public browser callback accepting an unverified identifier under its
existing privacy and activity gates. Signing does not authenticate account
selection, grant consent, encrypt IDs, prevent deletion, or prevent replay
of an intact signed cookie. Host identity-cookie imports remain unverified
identity hints. shared auction foundations must not upgrade any of these hints into user/account
authorization or a proof of consent.

Callback authentication needs an independently reviewed issuer/bidder
protocol. This change does not invent nonce semantics that bidders cannot
honor, add an Origin requirement to browser redirects, or substitute CORS
for write authorization. All existing permission and opt-out gates remain.

## Wire Envelope

The wire value is `v1.<key-id>.<payload>.<mac>`.

- `payload` is the existing bounded padded base64url JSON encoding. Its
  deterministic inner representation and reference fixtures remain useful
  payload qualification; they are not claims of signed-wire parity.
- `key-id` is 1-32 ASCII alphanumeric, underscore or hyphen bytes.
- `mac` is unpadded base64url of a full 32-byte HMAC-SHA256 result.
- The MAC input is the bytes of `auctioneer.uids.v1\0` followed by the exact
  `v1.<key-id>.<payload>` prefix. Version and key ID are authenticated too.
- Verify with RustCrypto's constant-time `Mac::verify_slice`, not string
  comparison or a hand-written cryptographic implementation.
- Require exactly four nonempty fields and canonical MAC encoding. Bound
  the complete envelope before splitting, key lookup or payload decoding.
- Only decode the inner payload after successful MAC verification. A valid
  MAC over malformed payload is still an invalid cookie, not a fresh one.

Tampering, malformed envelope/payload and unknown keys return a fixed
`400 invalid UID cookie` response without cookie writes. Existing byte/node
limit failures retain fixed resource-limit diagnostics. No raw token, UID,
keyring, key ID supplied by a client or provider error enters logs/debug.

## Key Source And Rotation

Use the existing default logical EdgeZero secret-store binding, not runtime
config, a new config loader, process-global mutable key cache, or another
provider client. The secret key name is `usersync_cookie_keyring`.

The secret is one atomic JSON value:

```json
{
  "active": "key-id",
  "keys": [{ "id": "key-id", "key": "unpadded-base64url-32-random-bytes" }]
}
```

Require 1-8 unique key IDs, exactly 32 decoded bytes per key, an active ID
present in the set, and no unknown/duplicate object fields. Reject oversized,
malformed or ambiguous keyrings with fixed errors. Key material is absent
from normal config, build artifacts, manifests, examples for production and
PR/commit output. Test-only fixtures use explicitly synthetic keys.

Load at most one keyring per sync request using
`BoundSecretStore::get_bytes_bounded`: a 4-KiB backend/value allowance and the
same request-clock absolute deadline used by the handler. The loader is not
called for preflight, status, info, auctions or health. Owned decoded keys
use zeroizing storage; provider-owned returned buffers and crypto internals
are not claimed to have stronger erasure guarantees than their libraries.

The SDK allowance bounds accepted guest values, not a universal peak for
provider materialization. At the pinned SDK revision, synchronous host reads
cannot always be interrupted, and some adapters inspect size after a whole
value is materialized. Keep the provisioned keyring within 4 KiB, account for
the qualified provider behavior, and recheck the deadline after the read.
Never claim that dropping the handler interrupts every host operation or
that an oversized provisioned secret had no transient allocation. These
limitations do not justify accepting or parsing an oversized returned value.

Missing bindings/secrets, malformed keyrings, provider failures and oversized
secret reads close UID processing with a fixed `503` and no cookie write.
Do not surface provider text. Keep this refusal in the existing aux/terminal
egress lifetime. Bound and charge keyring parsing, retained keys, secret
backend bytes, envelope formatting and MAC work in the aux derivation.

The config and keyring are each resolved once per request. Key availability
errors are deferred to the existing UID-read stage: host opt-out, malformed
request, disabled account and other earlier endpoint errors retain their
existing order. No failed secret read creates an unsigned-cookie path.

To rotate, atomically publish both keys with the new active ID. New writes
use the active key; reads accept every listed key. Keep the old key for the
operator's chosen cookie lifetime, then remove it to invalidate that key's
cookies. Emergency retirement invalidates cookies but cannot revoke copies
already consumed or prevent replay while their key remains accepted.

## Cookie Writes And Browser Bounds

Sign only the request-local candidate after all mutation/eviction decisions.
The transaction still attaches no Set-Cookie on refusal or partial failure.
Measure the complete signed envelope and all emitted cookie attributes,
not merely the inner JSON/base64 value. Zero logical size remains the absence
of an operator eviction threshold; it never disables hard limits.

Use a 4-KiB complete Set-Cookie hard ceiling for the signed transport so a
successful response cannot knowingly emit the former 16-KiB browser-unusable
field. Keep incoming aggregate-header/JSON limits independently bounded.
The hard ceiling is not a promise that third-party cookies are accepted:
browser policy and deployment topology still determine acceptance.

Clearing the UID map must explicitly expire the matching Path/Domain cookie
with Max-Age=0 and a past Expires timestamp. This replaces the previous
future-expiring empty value, which would be an invalid unsigned cookie on
the next request. Test deletion and subsequent browser readback; there is
no unsigned empty-payload identity state to accept on a later read.
Preserve current host opt-out precedence, UID expiry semantics, CORS,
no-store, and privacy/activity enforcement independently of integrity.

## Ownership And Verification

- `auctioneer-core::usersync::signed` owns the pure keyring/MAC envelope.
- The existing bounded cookie codec owns inner JSON/header grammar.
- Shared handlers own one bounded secret read and endpoint error mapping;
  generated EdgeZero actions and provider dispatch remain unchanged.
- Provider manifests/docs declare secret provisioning through their existing
  secret-store path, with no committed credentials or new workspace crate.

Required tests independently vary payload, version, key ID, MAC, secret
availability, rotation, opt-out, cookie size and source UID. Include an
independent fixed HMAC vector, actual generated-App and native-wire
callback/readback, tampered/unsigned cookie rejection on every reader,
key-store cancellation/deadline limits, redacted diagnostic assertions,
complete-field size boundaries and real-browser acceptance. Measure complete
handler allocation paths; compiling a provider is not live deployment proof.

This approved user sync addendum supersedes the earlier signing exclusion and
unsigned production-cookie wire contract. Encryption and authenticated
callback UID ownership remain outside this integrity boundary.

Sources: [RustCrypto HMAC](https://docs.rs/hmac/0.12.1/hmac/),
[zeroize guarantees](https://docs.rs/zeroize/1.8.2/zeroize/), and
[HTTP cookie integrity limitations](https://www.rfc-editor.org/rfc/rfc6265.html#section-8.6).
