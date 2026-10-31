# Reference Fixture Verification

Native, Rust-only, read-only validation of externally captured evidence. No
reference execution, codec port, writes,
network access, refresh, or acceptance command.

```sh
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root /path/to/worktree
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root /path/to/worktree --source /path/to/extracted-reference
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root /path/to/staged-worktree
```

`captures.json` pins all ten retained captures (734,948 bytes),
including privacy-policy and sync captures, GPP headers, source lock, original
license and pixel. The repository import renamed project references in sync
provenance notes, without changing captured inputs or expected outputs; see
`docs/repository-import.md`. Pins are compiled into the executable: a staged manifest
cannot replace them. Any supplied tree is validated against those same pins. Staging
needs the same relative capture paths, not a complete Rust checkout.

Provenance is checked independently of hashes. The 198 source-call cases retain
their exact original metadata and must have unique IDs and nonempty provenance
covered by the unchanged 35-path source lock. Older privacy/GPP captures have
historical metadata and attribution in their fixture READMEs; no new
per-row source provenance is invented. Optional source validation checks the
existing lock only, not all transitive modules or Git commit authenticity.

Reads are size-bounded; absolute/traversing/noncanonical paths and artifact
symlinks are rejected. A filesystem guard rejects `.go`, `.go.txt`, `go.mod`
and `go.sum`, excluding `.git`, `.claude` worktrees, `target` and `node_modules`.
Upstream paths and historical Go metadata inside captures remain legitimate.
Separate design divergences, codecs, PNG decoding and Rust-authored hostile
template controls remain the responsibility of the existing Rust consumers.
Operation input/expected shapes and row counts also belong to those consumers,
not this utility. The GPP vectors compile as typed Rust tuples; GPP parser tests
pin their count, unique headers, ordered IDs and decoded content. The utility
does not parse Rust syntax or implement a generic operation-schema engine.

The version-2 pin manifest contains only paths, SHA-256 digests and byte counts.
`reference check --root` is the sole reference command; `validate`, `refresh`
and the redundant `check-capture` command have no compatibility aliases.

Future reference recapture occurs externally. Retired tools are not part of
this fresh-history repository. Changes to captures require explicit evidence review and a reviewed
manifest change; this utility cannot generate or bless them.
