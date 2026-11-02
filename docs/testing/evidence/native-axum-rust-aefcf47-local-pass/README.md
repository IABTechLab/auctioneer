# First Rust Native Capture

Source: `aefcf47459eba3781e147f90fb4290d33518ebee`. The profile was
committed and pushed in `9a8ebef` before traffic. Its SHA-256 is
`de3e68dcefebe00cbda66317858f8e9823a06d29fdd51dc0d813cedeb6343aa2`.
The actual release CLI ran on October 15, 2026:

```sh
target/release/auctioneer-cli qualify native \
  --profile docs/testing/fixtures/resource-qualification/native-axum-rust-aefcf47.json \
  --output /private/tmp/auctioneer-rust-native-aefcf47-run \
  --source-commit aefcf47459eba3781e147f90fb4290d33518ebee --no-build
```

All 45 rows passed local acceptance with no recorded failures. Four real
A/B/corrupt/restored processes completed. Maximum process-lifetime high-water
was 49,807,360 bytes; maximum sampled retained growth was 15,974,400 bytes.
Accounting is 2,533,631,672 bytes against the unchanged 4-GiB platform with
256-MiB required headroom. All six SDK capability attestations remain pending;
overall qualification is unsupported and publisher readiness is false. Exit 1
is the specified completed-nonqualification result, not an overall pass.

## Separate Frozen Control Failed

The subsequent `qualify frozen-probe --evidence` command exited 2 with the exact
diagnostics retained in `frozen-probe-diagnostics.txt`. It reached the
invalid-consent control after the fourteen-version held-work phase. Source
inspection found that this negative control reused a valid generated proof for
a request carrying no consent. Missing consent correctly denies without a
malformed-consent warning, so that unrelated proof falsely earned parse credit.
This is a control attribution defect, not a production privacy grant.

The separate control is **not passed** by this capture. A correction must bind
credit to the actual supplied consent, use a new source/profile identity and
rerun both measurement and the explicit control. These artifacts and the old
profile remain unchanged; their passing rows do not complete that follow-up.

`SHA256SUMS` covers the retained files. Frozen executables remain only in the
local evidence directory, with their digests in profile/runner metadata. No raw
HTTP/config/consent bodies, logs, credentials or temporary stores are retained.
No credentialed provider test was run; M7.6 execution remains separate.
