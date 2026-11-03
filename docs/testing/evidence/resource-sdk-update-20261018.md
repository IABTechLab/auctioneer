# Resource SDK Update Verification

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-18

The bidder catalog branch updates IABTechLab/edgezero PR #275 from
`f6c708ccb5be19ba9fa92575db3c08c0f17e0d72` to
`bc300ace5ca1ffe7060594a7bdb01c1da80de305`. The primary repository's PR head
and comparison were fetched directly. The new head is one direct successor:
`docs: define outbound adoption requirements and implementation plans`.

The comparison adds seven Markdown documents and changes no runtime, adapter,
Cargo or toolchain files. Both revisions have identical `crates` tree object
`1e8f391582a8a89776a1a0cdf330b3a3b59b57d1` and root Cargo manifest object
`dd3b97b0923a83cc96a3ec9427b18fdd6acaf872`. This provides no new memory facts,
stack bound, enforcement hook or deployment qualification.

## Changes And Checks

All six workspace dependency declarations, eight lockfile SDK sources and
current-owned source/CI expectations now identify the new revision. Cargo
regenerated the lockfile: all 465 package versions, dependencies and checksums
are unchanged. Historical evidence and captures retain their original pins.

The existing pin test failed against the old manifest after its expected pin
was updated, before the manifest repair. With the new manifest and lockfile:

| Check                                                                                                                                                       | Result                                                 |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `cargo test --locked --offline -j 2 -p auctioneer-core --test codec_pins --test ci_workflows`                                                                   | 96 passed, 0 failed                                    |
| `cargo test --locked --offline -j 2 -p auctioneer-adapter-axum --test resource_profile artifact_identity_binds_compiled_config_references_features_and_sdk_pin` | 1 passed, 0 failed; 35 unrelated tests filtered        |
| Locked offline Cargo metadata and source inventory                                                                                                          | All eight SDK packages resolve to the exact new source |
| Formatting and diff whitespace checks                                                                                                                       | Passed                                                 |

Logs are retained locally under `/private/tmp/resource-qualification-sdk-update-*.log`. Existing
GVL and future-incompatibility warnings remain. This scoped verification did
not run the complete CLI suite, vendor packaging or WASM builds and does not
recertify old measurements. The current resource qualification fit failures and unknown provider
facts remain open.
