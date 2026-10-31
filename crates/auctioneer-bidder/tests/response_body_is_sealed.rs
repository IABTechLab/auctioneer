//! The node budget is STRUCTURAL, not conventional.
//!
//! `response_node_budget.rs` proves the budget is enforced. That is
//! necessary and not sufficient: it would pass equally against a design
//! where each bidder remembered to call a bounded parser, and the
//! whole point of sealing the body is that a bidder cannot forget.
//!
//! The stronger property, stated as a compiler question: **can code
//! holding a `&BidderHttpResponse` obtain its bytes?** If it can, the
//! bound is a convention — a new bidder, or a careless edit, parses
//! `&[u8]` with `serde_json` and the gap reopens silently, with nothing
//! failing. If it cannot, the bound holds by construction.
//!
//! Only the compiler can answer that, so this is a compile-fail
//! harness: child `cargo build`s over probe crates that depend on
//! `auctioneer-bidder` by path and try each route to the bytes a bidder
//! author might reach for. Every route must fail to compile with the
//! expected privacy, missing-getter or move-out-of-borrow diagnosis.
//!
//! # Why a compile-fail test and not a weaker guard
//!
//! There is no runtime assertion that expresses "this code does not
//! exist." A test can only exercise code that compiles, so a runtime
//! guard can show the parse refuses an over-budget body but never that
//! an alternative parse is unavailable. The property is about the
//! API surface, and the compiler is the only instrument that reads it.
//!
//! # What makes the verdict trustworthy
//!
//! The same two traps the `insecure_bidders_snapshot.rs` harness uses,
//! because "the build failed" is weak evidence alone:
//!
//!   1. The failure must match the route's expected compiler diagnosis
//!      and name the sealed field, getter or response type. A
//!      fixture that fails on a typo, a missing manifest key or a
//!      poisoned target dir fails this test rather than passing it.
//!   2. A CONTROL fixture using only the public surface must build
//!      clean through the identical path. If the control also fails,
//!      the harness is broken and says so rather than reporting its own
//!      breakage as the seal holding.
//!
//! Each probe gets its own PACKAGE NAME carrying its label and this
//! process's pid, because cargo fingerprints a build unit by package
//! name and feature set rather than by manifest path — two probes
//! sharing a name in one `--target-dir` are one unit, and a successful
//! case's binary would then satisfy a failing case's build.
//!
//! Helpers live INSIDE `mod tests` so they may use `expect` on setup
//! with no lint attribute: clippy.toml's `allow-expect-in-tests` reaches
//! items in a test module but not file-scope functions beside it.

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{self, Command};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Per-invocation nonce, so repeat runs of this binary cannot
    /// collide on a fixture directory.
    static FIXTURE_NONCE: AtomicU64 = AtomicU64::new(0);

    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn workspace_root() -> PathBuf {
        crate_dir()
            .parent()
            .and_then(Path::parent)
            .expect("crate two parents up = workspace root")
            .to_owned()
    }

    /// A probe crate depending on `auctioneer-bidder` by path, doing one
    /// thing in `main`.
    ///
    /// `[workspace]` is empty on purpose: without it cargo walks up,
    /// finds the real workspace manifest and refuses the probe as a
    /// member it does not list.
    struct Probe {
        dir: PathBuf,
        package: String,
    }

    impl Probe {
        fn new(label: &str, body: &str) -> Self {
            Self::for_dependency(label, body, "auctioneer-bidder", &crate_dir())
        }

        fn new_core(label: &str, body: &str) -> Self {
            Self::for_dependency(
                label,
                body,
                "auctioneer-core",
                &workspace_root().join("crates/auctioneer-core"),
            )
        }

        fn for_dependency(
            label: &str,
            body: &str,
            dependency_name: &str,
            dependency: &Path,
        ) -> Self {
            let nonce = FIXTURE_NONCE.fetch_add(1, Ordering::Relaxed);
            let package = format!("auctioneer-seal-probe-{label}-{}", process::id());
            let dir = env::temp_dir().join(format!("{package}.{nonce}"));
            let src = dir.join("src");
            fs::create_dir_all(&src).expect("create the probe source dir");
            fs::write(
                dir.join("Cargo.toml"),
                format!(
                    "[package]\n\
                     name = \"{package}\"\n\
                     edition = \"2024\"\n\
                     version = \"0.0.0\"\n\
                     publish = false\n\
                     \n\
                     [workspace]\n\
                     \n\
                     [dependencies]\n\
                     {dependency_name} = {{ path = {:?} }}\n",
                    dependency.display().to_string(),
                ),
            )
            .expect("write the probe manifest");
            fs::write(src.join("main.rs"), body).expect("write the probe main");
            Self { dir, package }
        }

        /// Build the probe and return (success, combined output).
        ///
        /// A reported success is checked against the artifact on disk: a
        /// `cargo build` that exits 0 having produced no binary is not
        /// evidence that a reference resolved, and this harness's whole
        /// verdict rests on "compiled" versus "did not".
        fn build(&self) -> (bool, String) {
            let target_dir = workspace_root().join("target").join("seal-probe");
            let binary = target_dir.join("debug").join(&self.package);
            drop(fs::remove_file(&binary));
            let output = Command::new(env!("CARGO"))
                .args(["build", "--offline", "-j", "2", "--manifest-path"])
                .arg(self.dir.join("Cargo.toml"))
                .arg("--target-dir")
                .arg(&target_dir)
                // The child must resolve everything from its own
                // manifest. An inherited target or flag set from the
                // OUTER run would let cases share a compilation.
                .env_remove("CARGO_BUILD_TARGET")
                .env_remove("RUSTFLAGS")
                .current_dir(&self.dir)
                .output()
                .expect("spawn the child cargo build");
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            assert!(
                !output.status.success() || binary.is_file(),
                "the child cargo build exited 0 but left no binary at {}; the \
                 harness cannot tell a sealed field from a build that did \
                 nothing.\n{combined}",
                binary.display(),
            );
            (output.status.success(), combined)
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Probe {
        fn drop(&mut self) {
            // Best effort: a leaked tempdir is noise, a panic in Drop
            // during an already-failing test hides the real verdict.
            drop(fs::remove_dir_all(&self.dir));
        }
    }

    /// Uses only the public surface a bidder is meant to have. The
    /// control answers "did the probe fail because of the seal, or
    /// because the harness is broken?".
    const CONTROL_BODY: &str = r#"
use auctioneer_bidder::BidderHttpResponse;
fn main() {
    let resp = BidderHttpResponse::new(200, vec![], b"{}".to_vec())
        .within_node_budget(64);
    // The sanctioned route: parse within the budget. Also touch the
    // accessors a bidder legitimately reads.
    let borrowed = &resp;
    let _ = borrowed.parse_bid_response();
    let _ = borrowed.body_is_empty();
    let _ = borrowed.body_len();
    let _ = borrowed.status();
    let _ = borrowed.headers();
    let _ = borrowed.project_debug_body(auctioneer_bidder::debug_projection::DebugBodySchema::OpenRtb);
    assert_eq!(borrowed.max_parsed_nodes(), 64);
    assert!(borrowed.body_equals(b"{}"));
    assert!(!borrowed.body_equals(b"[]"));
    // An owner can still relocate the body after borrowed comparisons.
    assert_eq!(resp.into_body(), b"{}");
}
"#;

    /// Which compiler refusal a route must produce.
    ///
    /// Carried per route rather than checked as one disjunction over
    /// every code, because the families of bypass fail for
    /// genuinely different reasons and a combined check would accept
    /// either error from either route. A `Clone` route that started
    /// failing with a privacy error, or a field route that started
    /// failing on a missing method, would mean the probe no longer
    /// tests what it says it does — and that is precisely the rot this
    /// harness exists to prevent.
    #[derive(Clone, Copy)]
    enum Diagnosis {
        /// The field or the struct literal is private: `E0616` for a
        /// read, `E0451` for a literal, or the prose form.
        Privacy,
        /// No borrowed getter may expose bytes that can be copied into
        /// a fresh response with a wider budget.
        NoBorrowedBytes,
        /// `clone()` on a `&BidderHttpResponse` cannot produce an
        /// owned response, so the consuming escape hatch stays out of
        /// reach.
        ///
        /// The code is `E0507` — "cannot move out of a shared
        /// reference" — and not the `E0599` a missing method would
        /// give, because `&T` implements `Clone` for every `T`:
        /// method resolution finds that impl, `resp.clone()` yields
        /// another `&BidderHttpResponse` rather than an owned one, and
        /// the refusal lands on the by-value call that follows.
        ///
        /// That is the bound this route needs. What made the bypass
        /// work was `Clone` on the TYPE handing back an owned value
        /// from a borrow; with the derive gone the only `clone` in
        /// scope returns a reference, and every consuming hatch is
        /// unreachable from a `&` again.
        NoOwnedCopy,
        /// Projection accepts neither a raw-byte callback nor a wider node cap.
        NoProjectionOverride,
    }

    impl Diagnosis {
        /// Whether `output` carries this diagnosis.
        fn matches(self, output: &str) -> bool {
            match self {
                Self::Privacy => {
                    output.contains("E0616")
                        || output.contains("E0451")
                        || output.contains("private field")
                        || output.contains("private fields")
                }
                Self::NoBorrowedBytes => {
                    output.contains("E0599")
                        && output.contains("no method named `body_bytes`")
                        && output.contains("BidderHttpResponse")
                }
                // `E0507` is the expected code and the reason is in
                // the variant's doc. The move must be refused ON the
                // response type, so the message has to name it — a bare
                // `E0507` from some unrelated line would otherwise
                // satisfy this.
                Self::NoOwnedCopy => {
                    output.contains("E0507")
                        && output.contains("cannot move out of")
                        && output.contains("BidderHttpResponse")
                }
                Self::NoProjectionOverride => {
                    output.contains("E0061") && output.contains("project_debug_body")
                }
            }
        }

        /// What a mismatch should say this route was supposed to prove.
        fn describe(self) -> &'static str {
            match self {
                Self::Privacy => "the privacy error the sealed field produces",
                Self::NoBorrowedBytes => {
                    "the missing `body_bytes` getter error (`E0599`) on `BidderHttpResponse`"
                }
                Self::NoOwnedCopy => {
                    "the move-out-of-a-shared-reference error that removing the `Clone` \
                     derive produces: `clone()` on a `&` can no longer hand back an \
                     owned response"
                }
                Self::NoProjectionOverride => {
                    "the projection arity error (E0061), rejecting a callback or wider budget"
                }
            }
        }
    }

    fn projection_routes() -> Vec<(&'static str, String, Diagnosis)> {
        let prelude = "use auctioneer_bidder::BidderHttpResponse;\n";
        vec![
            (
                "header-replacement",
                format!(
                    "{prelude}fn replace(resp: &mut BidderHttpResponse) {{\n\
                     \x20   resp.headers.clear();\n\
                     }}\nfn main() {{}}\n"
                ),
                Diagnosis::Privacy,
            ),
            (
                "eligibility-overwrite",
                format!(
                    "{prelude}fn permit(resp: &mut BidderHttpResponse) {{\n\
                     \x20   resp.debug_body_eligibility = auctioneer_bidder::debug_projection::DebugBodyEligibility::SingleTextValue;\n\
                     }}\nfn main() {{}}\n"
                ),
                Diagnosis::Privacy,
            ),
            (
                "projection-callback",
                format!(
                    "{prelude}fn steal(resp: &BidderHttpResponse) {{\n\
                     \x20   let _ = resp.project_debug_body(auctioneer_bidder::debug_projection::DebugBodySchema::OpenRtb, |bytes: &[u8]| bytes.to_vec());\n\
                     }}\nfn main() {{}}\n"
                ),
                Diagnosis::NoProjectionOverride,
            ),
            (
                "projection-rebudget",
                format!(
                    "{prelude}fn widen(resp: &BidderHttpResponse) {{\n\
                     \x20   let _ = resp.project_debug_body(auctioneer_bidder::debug_projection::DebugBodySchema::OpenRtb, u32::MAX);\n\
                     }}\nfn main() {{}}\n"
                ),
                Diagnosis::NoProjectionOverride,
            ),
        ]
    }

    /// Every route to the bytes a bidder author might reach for.
    ///
    /// One probe each, because a single fixture would stop at its first
    /// error and leave the later routes unexamined.
    fn byte_routes() -> Vec<(&'static str, String, Diagnosis)> {
        let prelude = "use auctioneer_bidder::BidderHttpResponse;\n";
        let mut routes = vec![
            // The direct read: what the bidders did before the seal.
            (
                "field-read",
                format!(
                    "{prelude}fn parse(resp: &BidderHttpResponse) {{\n\
                     \x20   let _: &[u8] = &resp.body;\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::Privacy,
            ),
            // Destructuring, which reaches a private field without
            // naming a getter.
            (
                "pattern-destructure",
                format!(
                    "{prelude}fn parse(resp: &BidderHttpResponse) {{\n\
                     \x20   let BidderHttpResponse {{ body, .. }} = resp;\n\
                     \x20   let _ = body;\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::Privacy,
            ),
            // Building a response with a budget of the author's
            // choosing via a struct literal, which would let a bidder
            // widen its own bound.
            (
                "struct-literal",
                format!(
                    "{prelude}fn main() {{\n\
                     \x20   let _ = BidderHttpResponse {{\n\
                     \x20       status: 200,\n\
                     \x20       headers: vec![],\n\
                     \x20       debug_body_eligibility: auctioneer_bidder::debug_projection::DebugBodyEligibility::Ineligible,\n\
                     \x20       body: vec![],\n\
                     \x20       max_parsed_nodes: u32::MAX,\n\
                     \x20   }};\n\
                     }}\n"
                ),
                Diagnosis::Privacy,
            ),
            // Overwriting the attached budget through a mutable
            // borrow — the bypass that a `pub` budget field would open.
            (
                "budget-overwrite",
                format!(
                    "{prelude}fn widen(resp: &mut BidderHttpResponse) {{\n\
                     \x20   resp.max_parsed_nodes = u32::MAX;\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::Privacy,
            ),
            (
                "borrowed-getter",
                format!(
                    "{prelude}fn steal(resp: &BidderHttpResponse) -> &[u8] {{\n\
                     \x20   resp.body_bytes()\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::NoBorrowedBytes,
            ),
            // Ordinary reconstruction must not recover the exchange's
            // sealed bytes and then use the sanctioned parser unbounded.
            (
                "borrowed-copy-rebudget",
                format!(
                    "{prelude}fn widen(resp: &BidderHttpResponse) -> BidderHttpResponse {{\n\
                     \x20   let body = resp.body_bytes().to_vec();\n\
                     \x20   let rebuilt = BidderHttpResponse::new(\n\
                     \x20       resp.status(), resp.headers().to_vec(), body,\n\
                     \x20   ).within_node_budget(u32::MAX);\n\
                     \x20   let _ = rebuilt.parse_bid_response();\n\
                     \x20   rebuilt\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::NoBorrowedBytes,
            ),
            // Cloning out of a `&` to reach the consuming `into_body`.
            //
            // The seal's original argument was that `into_body` takes
            // `self` by value, so a bidder holding a `&` cannot call
            // it. That is true and was not sufficient: while the type
            // derived `Clone`, `borrowed.clone()` produced the owned
            // value the signature asks for, and these two lines
            // compiled and ran. The route is spelled through a `&`
            // parameter, which is exactly what `make_bids` hands an
            // bidder.
            (
                "clone-into-body",
                format!(
                    "{prelude}fn steal(resp: &BidderHttpResponse) -> Vec<u8> {{\n\
                     \x20   resp.clone().into_body()\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::NoOwnedCopy,
            ),
            // Cloning out of a `&` to reach the consuming
            // `within_node_budget`, which replaces the exchange's
            // budget with one of the bidder's choosing. The more
            // dangerous of the two: the bytes alone are inert, while a
            // widened budget makes the SANCTIONED parse unbounded, so
            // a bidder could call `parse_bid_response` on the widened
            // copy and the bypass would look like the approved route.
            (
                "clone-widen-budget",
                format!(
                    "{prelude}fn widen(resp: &BidderHttpResponse) -> BidderHttpResponse {{\n\
                     \x20   resp.clone().within_node_budget(u32::MAX)\n\
                     }}\n\
                     fn main() {{}}\n"
                ),
                Diagnosis::NoOwnedCopy,
            ),
        ];
        routes.extend(projection_routes());
        routes
    }

    /// The harness can build a probe at all.
    #[test]
    fn the_harness_builds_a_public_surface_probe() {
        let control = Probe::new("control", CONTROL_BODY);
        let (ok, output) = control.build();
        assert!(
            ok,
            "the control probe uses only the public surface and must build; if \
             this fails the harness is broken and the seal verdicts beside it \
             mean nothing.\n{output}",
        );
    }

    /// No route reaches the bytes or the budget.
    ///
    /// This is the acceptance test for the whole design: a bidder
    /// author working inside `bidders/<name>/mod.rs` is UNABLE to parse a
    /// response outside the node budget, because the bytes are not
    /// reachable from there.
    #[test]
    fn no_route_reaches_the_sealed_body_or_its_budget() {
        let mut failures = Vec::new();
        for (label, body, diagnosis) in byte_routes() {
            let probe = Probe::new(label, &body);
            let (ok, output) = probe.build();
            if ok {
                failures.push(format!(
                    "route `{label}` compiled, so the node budget is a convention \
                     rather than a structural bound: code outside auctioneer-bidder can \
                     reach a bidder response's bytes and parse them unbounded.\n{output}"
                ));
                continue;
            }
            // The failure must be THE failure. Checking only the exit
            // status would accept a probe that broke for any other
            // reason, which is how a compile-fail assertion rots into a
            // test that passes whatever happens.
            if !diagnosis.matches(&output) {
                failures.push(format!(
                    "route `{label}` failed, but not with {}. A different error means \
                     the probe is broken rather than the bypass being closed.\n{output}",
                    diagnosis.describe(),
                ));
            }
            // And it must be about the SEALED surface, not some
            // neighbouring name. The `Clone` routes name the type
            // rather than a field, because what is missing there is an
            // impl on the type and not access to a member.
            let names_sealed_surface = output.contains("body")
                || output.contains("max_parsed_nodes")
                || output.contains("BidderHttpResponse");
            if !names_sealed_surface {
                failures.push(format!(
                    "route `{label}` failed with the right kind of error, but it named \
                     neither the sealed members nor `BidderHttpResponse`, so it is \
                     about something else.\n{output}"
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    }

    #[test]
    fn normalized_header_evidence_stays_paired_behind_read_only_access() {
        let prelude = "use auctioneer_core::outbound::BoundedProxyResponse;\n";
        let control = Probe::new_core(
            "core-control",
            &format!(
                "{prelude}fn inspect(response: &BoundedProxyResponse) {{ let _ = response.headers().len(); }}\nfn main() {{}}\n"
            ),
        );
        let (ok, output) = control.build();
        assert!(
            ok,
            "the core public-accessor control must compile: {output}"
        );
        for (label, operation, code, field) in [
            (
                "core-headers",
                "response.headers.clear();",
                "E0616",
                "headers",
            ),
            (
                "core-provenance",
                "let _ = response.content_type_provenance;",
                "E0616",
                "content_type_provenance",
            ),
            (
                "core-read-only",
                "response.headers().clear();",
                "E0596",
                "headers",
            ),
        ] {
            let probe = Probe::new_core(
                label,
                &format!(
                    "{prelude}fn replace(response: &mut BoundedProxyResponse) {{ {operation} }}\nfn main() {{}}\n"
                ),
            );
            let (compiled, diagnostic) = probe.build();
            assert!(!compiled, "{label} must not compile");
            assert!(
                diagnostic.contains(code) && diagnostic.contains(field),
                "{label} must fail on its intended header-seal guard: {diagnostic}"
            );
        }
    }
}
