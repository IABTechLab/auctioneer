# User Sync Codec Qualification

The reference suite evaluates existing libraries before the bounded UID
cookie implementation uses them. These dependencies are test-only until
that implementation lands; passing these checks does not qualify a sync
endpoint or its resource bounds.

## Selected Library Versions

- `base64 = 0.22.1`, default features off, `std` enabled. The workspace
  already resolved this version transitively.
- `cookie = 0.18.1`, all optional features off. No signed/encrypted jar,
  key derivation, randomness, or percent-decoding feature is enabled.
- Existing `serde_json = 1.0.150` and `time = 0.3.51` from `Cargo.lock`.
- Test-only `png = 0.17.16`, default features off, for complete decoding
  and chunk CRC validation of the unmodified upstream pixel.

Version-specific source and API documentation were inspected in the local
Cargo registry. The authoritative upstream behavior is captured by the
pinned Go reference harness, not inferred from Rust library defaults.

## Compatibility Boundaries

| Surface | Qualification result and implementation requirement |
| --- | --- |
| Base64 encoding | `URL_SAFE` emits the required padding and `-`/`_` alphabet. |
| Base64 decoding | Default Rust decoding rejects nonzero unused padding bits that Go accepts. Use an explicitly configured URL-safe engine with `with_decode_allow_trailing_bits(true)`, retaining required padding. Go also discards CR/LF; a bounded compatibility adapter must handle exactly those bytes, not trim arbitrary whitespace or guess alternative encodings. |
| Cookie order | Iterating every `HeaderMap::get_all(COOKIE)` entry, then `Cookie::split_parse`, preserves duplicates and first-match order for valid pairs. This alone does not qualify provider ingress preservation. |
| Cookie values | Use `value_trimmed` for quoted pairs; do not percent-decode UID cookie values. The Rust parser accepts invalid request-cookie names/backslashes/quotes, trims leading value spaces, and rejects Go's bare-name empty-value form. It is not a drop-in Go request parser. Validate and bridge the request grammar in the bounded cookie layer before treating parsing success as valid identity input. |
| Cookie formatting | The library emits the required attributes without HttpOnly when configured explicitly. Its attribute order differs from Go (SameSite/Secure precede Path/Domain/Expires). Pin the complete Rust output and compare reference attributes and total field-value size. Never build a header by interpolating unchecked values. |
| JSON escaping | `serde_json` retains UTF-8 and escapes JSON syntax. upstream's cookie encoder uses HTML-safe escaping, unlike its `/cookie_sync` response encoder. Cookie encoding needs a structured serializer compatibility adapter. Match each endpoint's reference fixtures independently; do not apply HTML escaping globally. |
| JSON shape | Missing/null/default and duplicate-field semantics must be driven by cookie fixtures, not by assuming a derived Rust struct matches Go. Count all syntactic nodes before retaining a last-value-wins map. |
| Timestamps | `time::Rfc3339` preserves tested fractional precision, explicit offsets, and year 0001. It rejects malformed calendar values. Decode fixtures determine Go's missing/null entry behavior; expiry comparison uses an injected clock. |

Do not promote these libraries to production dependencies until the cookie
module tests consume the reference corpus and cover the compatibility
adapters, corruption-versus-resource-limit distinction, and bounded output.
