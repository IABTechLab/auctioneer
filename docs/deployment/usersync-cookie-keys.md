# User-Sync Cookie Keys

Provision the secret named `usersync_cookie_keyring` through the existing
default logical EdgeZero secret-store binding (`auctioneer_secrets` in the shipped
manifest). Do not add a store
ID, put keys in runtime/account config, or bake them into manifests or builds.
The secret is one complete JSON value; do not publish `active` and individual
keys as separate secrets.

## Keyring Format

This is a schema example, not a usable keyring or credential:

```json
{
  "active": "current",
  "keys": [
    { "id": "current", "key": "<unpadded-base64url-of-32-random-bytes>" }
  ]
}
```

- The entire UTF-8 JSON value, including whitespace, must be at most **4096 bytes**.
- `keys` contains **1-8** entries with unique IDs. `active` must name a listed ID.
- Each ID is **1-32 ASCII characters**, using only `A-Z`, `a-z`, `0-9`, `_`, or `-`.
- Each key is **32 cryptographically random bytes**, encoded as unpadded
  base64url: exactly **43 characters**, with no `=` padding. Encoding is not
  encryption. Generate key material outside the repository with an approved
  secret manager or cryptographically secure random generator.
- Unknown or duplicate JSON fields, duplicate IDs, malformed encoding, missing
  active keys, and oversized keyrings are rejected. Never use placeholders,
  all-zero keys, or test fixtures in production.

Treat the entire keyring as a credential. Do not put it in logs, command-line
arguments, shell history, source control, build output, tickets, or commit text.
Use secure provider injection or secret upload, not an inline JSON argument.

## Provider Provisioning

These lookups follow the pinned EdgeZero revision
`f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`. The same secret name has different
provider-specific provisioning surfaces:

| Adapter               | Provisioning surface                                                                     |
| --------------------- | ---------------------------------------------------------------------------------------- |
| Axum `EnvSecretStore` | Process variable `EDGEZERO__SECRETS__USERSYNC_COOKIE_KEYRING`                            |
| Cloudflare            | Worker secret binding `usersync_cookie_keyring`                                          |
| Spin                  | Application variable `usersync_cookie_keyring`, mapped into the component                |
| Fastly production     | Secret Store item `usersync_cookie_keyring`, resource-link alias `auctioneer_secrets`    |
| Fastly local Viceroy  | Same store/item; the manifest's example env source is `AUCTIONEER__USERSYNC__COOKIE_KEYRING` |

### Axum

Inject the full JSON value into `EDGEZERO__SECRETS__USERSYNC_COOKIE_KEYRING`
before starting the process. The pinned
[EnvSecretStore](https://github.com/IABTechLab/edgezero/blob/f6c708ccb5be19ba9fa92575db3c08c0f17e0d72/crates/edgezero-adapter-axum/src/secret_store.rs)
maps the lowercase secret key to `EDGEZERO__SECRETS__` plus its ASCII-uppercase
form. Neither the bare lowercase key nor the Fastly example env name is an
Axum alias. Use deployment secret injection; the environment-backed adapter
does not itself provide encrypted storage.

### Cloudflare Workers

From the adapter's Wrangler project, provision the selected Worker/environment
using the interactive secret prompt:

```sh
wrangler secret put usersync_cookie_keyring
```

Supply the full JSON keyring, not an individual key. Provision secrets separately
for each deployment environment. Keep the value out of `[vars]` and out of the
checked-in Wrangler manifest. See [Worker secrets](https://developers.cloudflare.com/workers/configuration/secrets/).

The pinned
[CloudflareSecretStore](https://github.com/IABTechLab/edgezero/blob/f6c708ccb5be19ba9fa92575db3c08c0f17e0d72/crates/edgezero-adapter-cloudflare/src/secret_store.rs)
calls `Env::secret(key)` unchanged: **lowercase `usersync_cookie_keyring`** is the
binding name. There is no Axum prefix or uppercase conversion. The SDK secret
namespace is flat; the logical default binding does not require a new Worker
store or namespace.

### Spin

The shipped `spin.toml` declares:

```toml
[variables]
usersync_cookie_keyring = { required = false, default = "", secret = true }

[component.auctioneer-adapter-spin.variables]
usersync_cookie_keyring = "{{ usersync_cookie_keyring }}"
```

Provide the full JSON value through a secret-capable application-variable
provider. For local development, Spin's environment provider uses
`SPIN_VARIABLE_USERSYNC_COOKIE_KEYRING` on the **Spin process**, not the
component's `environment` table. Both application declaration and component
mapping are necessary. See [Spin application variables](https://spinframework.dev/v4/variables).

The pinned
[SpinSecretStore](https://github.com/IABTechLab/edgezero/blob/f6c708ccb5be19ba9fa92575db3c08c0f17e0d72/crates/edgezero-adapter-spin/src/secret_store.rs)
lowercases the lookup key without stripping underscores. Keep the literal
`usersync_cookie_keyring` name. Spin requires every non-required variable to
have a default. The empty default permits deployment without a keyring, but
is not valid keyring JSON: UID processing returns signing-unavailable with no
cookie write. It never enables unsigned processing or supplies a signing key.
The health token likewise has an empty secret default and remains closed
unless an operator configures access. The vendor CI test drives Spin 4.1.0
with absent and supplied secrets and rejects removal of either default; it
validates manifest loading, not guest execution or secret-provider delivery.

### Fastly

In production, link the existing Secret Store under the alias selected by
`[stores.secrets].default` (`auctioneer_secrets` in the shipped manifest).
Add or replace the item `usersync_cookie_keyring` with the entire
JSON value using secure upload. Keep `health_gvl_token` intact in that store.
The pinned
[FastlySecretStore](https://github.com/IABTechLab/edgezero/blob/f6c708ccb5be19ba9fa92575db3c08c0f17e0d72/crates/edgezero-adapter-fastly/src/secret_store.rs)
opens the logical store and looks up the literal key. See
[Fastly Secret Stores](https://www.fastly.com/documentation/guides/compute/edge-data-storage/working-with-secret-stores/).

The local `fastly.toml` adds an env-backed item under the same default store.
`AUCTIONEER__USERSYNC__COOKIE_KEYRING` is a documentary example host-env source for
Viceroy only: it is **not** a new Auctioneer runtime config key, production binding,
or direct handler environment lookup. No value is checked in. Viceroy uses an
empty value when the env source is unset, which is not a valid signing keyring.
Local settings do not provision production secrets. See the
[Fastly manifest format](https://www.fastly.com/documentation/reference/compute/fastly-toml/).

Health fixtures must preserve `health_gvl_token` and its existing env source.
Env snapshots/child-process fixtures should leave the new keyring env source
unset unless explicitly testing signing; never inherit an operator's keyring
into test fixtures. Adding this item must not make health depend on cookie keys.

## Rotation And Retirement

1. Generate a fresh key outside the repository and give it a new unique ID.
2. Atomically publish a complete keyring containing old and new keys, keeping
   the old ID active. Confirm readers across regions, replicas, and rollback
   versions can see both keys before changing the active ID. A whole-value
   update is not a guarantee of simultaneous fleet-wide propagation.
3. Atomically replace the keyring with the same retained keys and the new ID
   active. New cookies use the active key; reads accept every listed key.
4. Retain the old key for the chosen cookie acceptance lifetime and rollout
   overlap. Then remove it with another whole-keyring update. Cookies bearing
   that retired or unknown ID are hard-rejected, not decoded as unsigned state.

Do not reuse an ID for different key bytes or discard the active key. Stay
within eight keys and 4096 bytes throughout rotation. Emergency retirement
invalidates the affected cookies once readers observe the new keyring; it
does not undo past use or prevent replay while a key remains accepted.

## Failure And Size Boundaries

There is **no unsigned-cookie fallback** and no implicit migration that trusts
legacy unsigned UIDs. Missing bindings, missing/empty/invalid keyrings, provider
errors, oversized reads, or deadline expiry refuse UID processing with a fixed
signing-unavailable response and no cookie write. Earlier endpoint errors and
opt-out checks retain their order. Health and non-sync routes do not need this
keyring.

A signature protects integrity of issued cookie state, not the truth of an ID.
The public callback UID remains **unverified caller input**. Signing is not
callback authentication, consent, authorization, or replay prevention.

The signed cookie has a hard **4096-byte complete Set-Cookie field-value
ceiling**, including the cookie name, signed envelope, and emitted attributes.
The key ID and MAC count toward this ceiling. A zero configured eviction
threshold does not disable the hard cap. Cookie failure attaches no partial
write; a small valid cookie still is not a promise of browser acceptance.
This replaces the old 16-KiB output allowance without a compatibility mode:
requested `hard_cap_bytes` above 4096 are clamped to 4096, and lower limits
still apply. Boundary tests cover lower, equal, and higher requested limits
against the complete field, not only the signed payload.

One keyring is loaded per sync request using the existing bounded secret read,
a 4096-byte backend/value allowance, and the request's absolute deadline. These
bounds limit **accepted secret bytes**, not every provider's transient allocation:

- Axum's environment lookup materializes an OS value before checking its size.
- Cloudflare converts the whole Worker secret to a string before size checks;
  deadline checks are cooperative around the read.
- Spin returns a whole variable string before byte checks. Its timer can bound
  guest return, without proving cancellation of the underlying host operation.
- Fastly's synchronous lookup/decryption can materialize the full plaintext
  before the byte check; deadline checks cannot interrupt every host call.

Provision only values within 4096 bytes even if the platform permits more.
Oversized returned values must not reach parsing or signing. Do not claim
zero transient allocation, universal host cancellation, or provider-buffer
zeroization; owned decoded signing keys are zeroized, but other buffers and
cryptographic internals retain their libraries' guarantees. The
[cookie-integrity design](../superpowers/specs/2026-10-13-usersync-cookie-integrity-design.md)
defines the shared boundary independently of provider materialization limits.
