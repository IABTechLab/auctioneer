//! Capture source-grounded ownership facts without inventing physical measurements.

use super::super::super::Error;
use super::super::{AdapterRow, Fact, Report, Selection};
use super::{
    AdapterAudit, Audit, ChangeClass, Component, ComponentReceipt, Decision, Evidence, Ownership,
    PendingFact, RuntimeFacts, Subject, TermId, component_values, derive_aux_work_for_profile,
    term_population, validate_audit,
};
use auctioneer_runtime::IsolateTerm;
use std::fmt::Write as _;

pub(super) fn capture(snapshot: Report) -> Result<Audit, Error> {
    super::require(
        snapshot.accounting_complete
            && snapshot.schema_version == 1
            && snapshot.selection == Selection::AllAdapters,
    )?;
    let first = &snapshot
        .adapters
        .first()
        .ok_or(Error::ResourceInput)?
        .identity;
    let mut audit = Audit {
        schema_version: 1,
        code_base_commit: first.code_base_commit.clone(),
        sdk_revision: first.sdk_revision.clone(),
        lock_sha256: first.lock_sha256.clone(),
        toolchain: first.compiler_verbose.clone(),
        features: first.features.clone(),
        runtime_facts: RuntimeFacts::current(),
        adapters: Vec::new(),
    };
    for report in snapshot.adapters {
        let auxiliary_receipts = derive_aux_work_for_profile(
            &report.effective_limits,
            &report.target_constants,
            report.baked_config_bytes,
        )
        .map_err(|_error| Error::ResourceInput)?
        .charges();
        let component_receipts: Vec<_> =
            component_values(&report, auxiliary_receipts, &audit.runtime_facts)?
                .into_iter()
                .map(|(component, bytes)| ComponentReceipt { component, bytes })
                .collect();
        let mut owners = Vec::new();
        let mut decisions = Vec::new();
        for term in IsolateTerm::ALL.iter().copied() {
            let source = ownership(term);
            owners.push(capture_owner(term, &report, &audit.runtime_facts)?);
            decisions.push(Decision {
                subject: Subject::Term { term_id: TermId(term) },
                class: ChangeClass::Unchanged,
                owner: source.owner.to_owned(),
                rationale: "Retain the complete charged term; source inspection alone authorizes no smaller coefficient or population".to_owned(),
                next_action: source.proof.to_owned(),
            });
        }
        for receipt in &component_receipts {
            if super::significant(receipt.bytes, report.derived.total_bytes) {
                let source = ownership(component_owner(receipt.component));
                decisions.push(Decision {
                    subject: Subject::Component { component: receipt.component },
                    class: ChangeClass::Unchanged,
                    owner: source.owner.to_owned(),
                    rationale: "This scaled component exceeds 5% of total demand; keep it funded until its owning bound is independently replaced".to_owned(),
                    next_action: source.proof.to_owned(),
                });
            }
        }
        let pending_facts = report
            .validation
            .external_blockers
            .iter()
            .map(|fact| {
                let (owner, action) = blocker_action(fact);
                PendingFact {
                    fact: fact.clone(),
                    reason: format!("The current source report remains blocked by {fact}"),
                    owner: owner.to_owned(),
                    next_action: action.to_owned(),
                }
            })
            .collect();
        let gap_bytes = report.validation.gap_bytes.map_or_else(
            || pending("Unknown effective platform ceiling prevents a numeric fit-gap claim",
                "AG/operator and EdgeZero adapter maintainer", "Supply the missing ceiling and scope; preserve the independent configured-cap refusal"),
            |value| Evidence::Known { value, source: "Runtime additive total plus selected headroom against the effective ceiling".to_owned() });
        audit.adapters.push(AdapterAudit {
            report, auxiliary_receipts, component_receipts, owners, decisions, pending_facts, gap_bytes,
            artifact: pending("This capture inspects source declarations, not a deployed or guest artifact",
                "Auctioneer target maintainer", "Build the selected target and record the artifact digest, ABI, compiler and covered paths"),
            abi: pending("Native layout substitutions do not prove target heap, host copies or separate stack",
                "Auctioneer target maintainer", "Execute an artifact-bound guest/host ownership measurement and prove each funded upper bound"),
        });
    }
    // Use exactly the same validator as imported records, including clean-source,
    // complete owner/term/component coverage and current runtime arithmetic.
    let bytes = toml::to_string(&audit).map_err(|_error| Error::Output)?;
    validate_audit(bytes.as_bytes())
}

fn capture_owner(
    term: IsolateTerm,
    report: &AdapterRow,
    facts: &RuntimeFacts,
) -> Result<Ownership, Error> {
    let source = ownership(term);
    let population = if term == IsolateTerm::PreAdmissionParser
        && matches!(
            report.platform_facts.inbound_population,
            Fact::Pending { .. }
        ) {
        pending(
            "The declared parser population has no published inbound bound",
            "EdgeZero adapter maintainer/provider",
            "Publish and enforce an ingress population at the selected memory scope",
        )
    } else {
        Evidence::Known {
            value: term_population(term, report, facts)?,
            source: source.path.to_owned(),
        }
    };
    let scope = match report.platform_facts.ceiling {
        Fact::Known { .. } => Evidence::Known {
            value: report.memory_scope.clone(),
            source: "Selected SDK platform metadata; dispatch alignment remains a separate blocker"
                .to_owned(),
        },
        Fact::Pending { .. } => pending(
            "The SDK does not publish the memory ceiling scope",
            "AG/operator and EdgeZero adapter maintainer",
            "Publish operator declarations and the enforced accounting scope",
        ),
    };
    Ok(Ownership {
        term_id: TermId(term),
        allocation_owner: source.owner.to_owned(),
        source_path: source.path.to_owned(),
        acquire: source.acquire.to_owned(),
        release: source.release.to_owned(),
        population,
        scope,
        enforcement: if source.enforced {
            Evidence::Known {
                value: source.enforcement.to_owned(),
                source: source.path.to_owned(),
            }
        } else {
            pending(source.enforcement, source.owner, source.proof)
        },
        overlapping_copies: source.overlap.to_owned(),
        abi_bound: pending(
            "No artifact-bound physical allocation upper bound is supplied by this source audit",
            source.owner,
            source.proof,
        ),
        disposition: ChangeClass::Unchanged,
    })
}

fn pending<T>(reason: &str, owner: &str, next_action: &str) -> Evidence<T> {
    Evidence::Pending {
        reason: reason.to_owned(),
        owner: owner.to_owned(),
        next_action: next_action.to_owned(),
    }
}

struct Description {
    owner: &'static str,
    path: &'static str,
    acquire: &'static str,
    release: &'static str,
    overlap: &'static str,
    enforced: bool,
    enforcement: &'static str,
    proof: &'static str,
}

fn ownership(term: IsolateTerm) -> Description {
    match term {
        IsolateTerm::Baseline => Description {
            owner: "Auctioneer target/core maintainers",
            path: "crates/auctioneer-core/src/bidder_registry.rs",
            acquire: "Cold initialization of defaults, registry and singleton state",
            release: "Instance or execution teardown",
            overlap: "Existing startup reserves remain additive with admitted work",
            enforced: false,
            enforcement: "A declared baseline is not an allocator byte gate",
            proof: "Measure cold and warm target residency, with initialization and failure paths identified separately",
        },
        IsolateTerm::Gvl => Description {
            owner: "Auctioneer privacy maintainer",
            path: "crates/auctioneer-privacy/src/gvl.rs",
            acquire: "GvlStore::get parses and caches a version; requests clone a handle",
            release: "Last cache or request handle, including requests pinning evicted versions",
            overlap: "Cache entries plus distinct auction-held and auxiliary-held versions; handles are not duplicate parsed lists",
            enforced: true,
            enforcement: "GVL_PARSE_CACHE_MAX_ENTRIES plus auction and auxiliary admission populations",
            proof: "Prove target parsed-list bounds and eviction with multiple distinct request-pinned versions",
        },
        IsolateTerm::Auctions => Description {
            owner: "Auctioneer core/exchange maintainers",
            path: "crates/auctioneer-core/src/handlers/auction.rs",
            acquire: "Auction admission and all six phase receipts before config extraction",
            release: "Serialization completion or cancellation drops the work receipts",
            overlap: "Config, decoded ingress, privacy projections, outbound requests and completed results can coexist",
            enforced: true,
            enforcement: "AdmissionGate and prepaid AuctionBudget phase receipts",
            proof: "Bound each phase on the target ABI at two sizes and unequal populations, including late failure and cancellation",
        },
        IsolateTerm::AuctionEgress => Description {
            owner: "Auctioneer core/target maintainers",
            path: "crates/auctioneer-core/src/handlers/auction.rs",
            acquire: "Egress reservation before response serialization",
            release: "Terminal response completion or cancellation, not work completion",
            overlap: "Serialized body, provider handoff copies and transmit state can outlive auction work",
            enforced: true,
            enforcement: "EgressGate lease bound to terminal completion",
            proof: "Exercise a slow terminal handoff and cancellation while new auctions are admitted",
        },
        IsolateTerm::Auxiliary => Description {
            owner: "Auctioneer core auxiliary-route maintainer",
            path: "crates/auctioneer-core/src/ingress.rs",
            acquire: "Auxiliary route admission/work reservation or independent probe-config work",
            release: "Route completion versus probe config resolution/render/publication; counters are independent",
            overlap: "Auxiliary work and max(auxiliary, probe minimum) config work coexist with output handoff",
            enforced: true,
            enforcement: "AuxGate, AuxWorkBudget and independent probe-config permits",
            proof: "Prove preboot/App-retained config defaults first, then cookie, signing, privacy, chooser, output and host-copy overlap on every auxiliary route",
        },
        IsolateTerm::TerminalEgress => Description {
            owner: "Auctioneer core/target maintainers",
            path: "crates/auctioneer-core/src/ingress.rs",
            acquire: "Terminal/static output reservation, including detached head refusals",
            release: "Response completion or cancellation",
            overlap: "Refusal output and transmit state are independent of auction and auxiliary work",
            enforced: true,
            enforcement: "StaticEgressGate terminal lease",
            proof: "Exercise preboot and saturated-class refusal output through actual target handoff",
        },
        IsolateTerm::ProbeOutput => Description {
            owner: "Auctioneer core health maintainer",
            path: "crates/auctioneer-core/src/ingress.rs",
            acquire: "Independent probe-output reservation, including preboot responses",
            release: "Output completion; the cached verdict remains resident independently",
            overlap: "Live probe output population plus one cached verdict, not probe-config work count",
            enforced: true,
            enforcement: "Probe egress permits and bounded cached response",
            proof: "Measure cached verdict plus live preboot/boot probe bodies at concurrent terminal completion",
        },
        IsolateTerm::PreAdmissionParser => Description {
            owner: "EdgeZero adapter maintainer/provider",
            path: "crates/auctioneer-runtime/src/budget.rs",
            acquire: "Host/framing/head parsing before Auctioneer route admission",
            release: "Host/parser cleanup, including rejected and cancelled input",
            overlap: "No Auctioneer admission counter bounds the pre-admission population; outbound connections are not inbound requests",
            enforced: false,
            enforcement: "Host ingress/framing accounting is unpublished at the pinned SDK",
            proof: "Publish and negatively test provider ingress population, framing, parser and host-storage bounds at the memory scope",
        },
        IsolateTerm::BodyStreams => Description {
            owner: "Auctioneer outbound/target maintainers",
            path: "crates/auctioneer-core/src/outbound/wave.rs",
            acquire: "Global wave stream lease before dispatch and decode",
            release: "Wave settlement, EOF or cancellation",
            overlap: "Completed response slots survive stream release across waves and remain in auction response funding",
            enforced: true,
            enforcement: "StreamGate wave leases and bounded decoder chain",
            proof: "Drive multiple waves with retained completed results, decoder windows and cancellation on the guest ABI",
        },
        IsolateTerm::CatalogResident => CATALOG_RESIDENT,
        IsolateTerm::CatalogInitialization => CATALOG_INITIALIZATION,
    }
}

const CATALOG_RESIDENT: Description = Description {
    owner: "Auctioneer catalog maintainer",
    path: "crates/auctioneer-core/src/bidder_registry.rs",
    acquire: "Incremental generated catalog/validator residency at initialization",
    release: "Instance or execution teardown",
    overlap: "Zero incremental descriptor does not remove existing registry residency funded by baseline",
    enforced: false,
    enforcement: "Catalog descriptor is an allocation bound, not an independent allocator gate",
    proof: "Measure real generated descriptors against the matched pre-catalog baseline without installation in the pure profile test",
};

const CATALOG_INITIALIZATION: Description = Description {
    owner: "Auctioneer catalog maintainer",
    path: "crates/auctioneer-core/src/bidder_registry.rs",
    acquire: "Incremental catalog initialization scratch, including validation failure",
    release: "Initialization completion or error cleanup",
    overlap: "Initialization remains an additive term even when no auction is currently executing",
    enforced: false,
    enforcement: "No incremental scratch is claimed by the current zero descriptor",
    proof: "Measure successful and failing catalog initialization separately from retained descriptors on the target ABI",
};

fn blocker_action(fact: &str) -> (&'static str, &'static str) {
    match fact {
        "unknown_memory_ceiling" | "unknown_scope" => (
            "AG/operator and EdgeZero adapter maintainer",
            "Publish operator ceiling and memory scope through the SDK and exercise configured overflow independently",
        ),
        "unknown_inbound_request_population" => (
            "EdgeZero adapter maintainer/provider",
            "Publish or enforce a scope-bound inbound population; do not reuse the outbound connection limit",
        ),
        "unknown_host_ingress_memory_accounting" => (
            "EdgeZero adapter maintainer/provider",
            "Publish and test host ingress/framing allocation ownership and bounds including cancelled and refused requests",
        ),
        "missing_proved_nonzero_separate_stack_requirement" => (
            "Auctioneer Fastly maintainer",
            "Prove a nonzero artifact/ABI/compiler-bound stack upper bound covering initialization, admitted depth, errors and cancellation",
        ),
        "platform_dispatch_mismatch" => (
            "Auctioneer target maintainer",
            "Align actual dispatch metadata and proof with the adapter-selected deployment before treating its platform facts as applicable",
        ),
        _ => (
            "Auctioneer resource maintainer",
            "Investigate this source-bound SDK blocker and provide an explicit owning proof before acceptance",
        ),
    }
}

fn component_owner(component: Component) -> IsolateTerm {
    match component {
        Component::AuctionConfig
        | Component::AuctionIngress
        | Component::AuctionPrivacy
        | Component::AuctionProjection
        | Component::AuctionOutbound
        | Component::AuctionResponse
        | Component::RuntimeConfig
        | Component::RequestSide
        | Component::CallSide
        | Component::Bookkeeping
        | Component::Debug
        | Component::Assembly => IsolateTerm::Auctions,
        Component::AuxConfig
        | Component::AuxBody
        | Component::AuxQuery
        | Component::AuxInputHeaders
        | Component::AuxCookie
        | Component::AuxSigning
        | Component::AuxNodes
        | Component::AuxPrivacy
        | Component::AuxChooser
        | Component::AuxUrls
        | Component::AuxRetainedUrls
        | Component::AuxOutput
        | Component::AuxDeclaredWork
        | Component::AuxHandoff
        | Component::AuxTransmit
        | Component::AuxRoutes
        | Component::ProbeConfig => IsolateTerm::Auxiliary,
    }
}

fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn amount(value: Option<u64>) -> String {
    value.map_or_else(|| "pending".to_owned(), |bytes| bytes.to_string())
}

fn label(value: &impl serde::Serialize) -> Result<String, Error> {
    serde_json::to_value(value)
        .map_err(|_error| Error::Output)?
        .as_str()
        .map(str::to_owned)
        .ok_or(Error::Output)
}

pub(super) fn render(audit: &Audit) -> Result<String, Error> {
    let mut output = String::from(
        "# Adapter Resource Envelopes\n\nGenerated from the canonical typed source audit. No remediation package is approved by this command. This is not a physical target measurement or fit sign-off.\n\n",
    );
    writeln!(output, "Code baseline: `{}`. SDK: `{}`. Lock: `{}`.\n\nFeatures: `{}`.\n\nCompiler:\n\n```text\n{}\n```\n", audit.code_base_commit, audit.sdk_revision, audit.lock_sha256, audit.features.join(", "), audit.toolchain.trim_end()).map_err(|_error| Error::Output)?;
    output.push_str("The equation is the runtime's checked additive sum; receipts are already population-scaled. Headroom is a separate acceptance inequality. Pending platform facts are not numeric zero.\n\n| Adapter | Demand Bytes | Headroom Bytes | Gap Bytes | Fit |\n|---|---:|---:|---:|---|\n");
    for target in &audit.adapters {
        let row = &target.report;
        writeln!(
            output,
            "| {} | {} | {} | {} | {} |",
            label(&row.adapter)?,
            row.derived.total_bytes,
            amount(row.validation.selected_headroom_bytes),
            amount(row.validation.gap_bytes),
            if row.required_fit {
                "source arithmetic only"
            } else {
                "blocked"
            }
        )
        .map_err(|_error| Error::Output)?;
    }
    for target in &audit.adapters {
        let row = &target.report;
        writeln!(output, "\n## {}\n\nProfile `{}`; dispatch `{}`; layout `{}`; reporter `{}`; derived compiler target `{}`.\n\nProfile digest: `{}`.\n\n| Term | Bytes | Allocation Owner | Acquire | Release | Overlap |\n|---|---:|---|---|---|---|", label(&row.adapter)?, row.platform_profile, row.dispatch_profile, label(&row.identity.layout_evidence)?,
            row.identity.reporter_abi, row.identity.target_abi, row.effective_profile_sha256).map_err(|_error| Error::Output)?;
        for owner in &target.owners {
            writeln!(
                output,
                "| `{}` | {} | [{}]({}) | {} | {} | {} |",
                owner.term_id.0.name(),
                row.terms
                    .get(owner.term_id.0.name())
                    .ok_or(Error::ResourceInput)?,
                cell(&owner.allocation_owner),
                relative_source(&owner.source_path),
                cell(&owner.acquire),
                cell(&owner.release),
                cell(&owner.overlapping_copies)
            )
            .map_err(|_error| Error::Output)?;
        }
        writeln!(output, "\nArtifact proof: {}. Target ABI bound: {}. Known owner ABI bounds: {}/{}. The charged population is not evidence of provider ingress enforcement.\n", evidence_status(&target.artifact), evidence_status(&target.abi), target.owners.iter().filter(|owner| owner.abi_bound.is_known()).count(), target.owners.len()).map_err(|_error| Error::Output)?;
        output.push_str("\n### Components\n\nThese are explanatory, already-scaled ledger entries, not additional summands. Nested phase/subcomponent rows must not be added to their parents.\n\n| Component | Bytes | Above 5% Of Demand |\n|---|---:|---|\n");
        for receipt in &target.component_receipts {
            writeln!(
                output,
                "| `{}` | {} | {} |",
                label(&receipt.component)?,
                receipt.bytes,
                if super::significant(receipt.bytes, row.derived.total_bytes) {
                    "yes"
                } else {
                    "no"
                }
            )
            .map_err(|_error| Error::Output)?;
        }
        output.push_str("\n### Next Proofs\n\n| Term | Disposition | Owner | Required Proof |\n|---|---|---|---|\n");
        for decision in &target.decisions {
            if let Subject::Term { term_id } = decision.subject {
                writeln!(
                    output,
                    "| `{}` | {} | {} | {} |",
                    term_id.0.name(),
                    label(&decision.class)?,
                    cell(&decision.owner),
                    cell(&decision.next_action)
                )
                .map_err(|_error| Error::Output)?;
            }
        }
        output.push_str(
            "\n### External Blockers\n\n| Fact | Owner | Next Verification |\n|---|---|---|\n",
        );
        for fact in &target.pending_facts {
            writeln!(
                output,
                "| `{}` | {} | {} |",
                cell(&fact.fact),
                cell(&fact.owner),
                cell(&fact.next_action)
            )
            .map_err(|_error| Error::Output)?;
        }
    }
    output.push_str("\n## Remediation Checkpoint\n\nThe audit freezes no projected savings. A concrete package must name exact changed owners, independent bounds, ordered deltas and residual gaps. AG must approve its immutable review commit before production ownership, admission or workload changes. Host facts, Fastly stack proof and the selected Spin dispatch remain independent prerequisites.\n\nSee [source investigation and rejected shortcuts](evidence/resource-envelope-audit-20261018.md) and the [implementation plan](../superpowers/plans/2026-10-17-resource-envelope.md).\n");
    super::require(output.len() <= super::super::MAX_BYTES)?;
    Ok(output)
}

fn relative_source(path: &str) -> String {
    format!("../../{path}")
}

const fn evidence_status<T>(evidence: &Evidence<T>) -> &'static str {
    match evidence {
        Evidence::Known { .. } => "known",
        Evidence::Pending { .. } => "pending",
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Evidence, Mode, tests as fixtures, validate_audit_mode};
    use super::*;
    use crate::bidders::resources::AdapterId;
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn source_audit_covers_every_runtime_owner_and_component_without_claiming_measurement() {
        let reports = fixtures::fixture()
            .adapters
            .into_iter()
            .map(|target| target.report)
            .collect();
        let audit = capture(super::super::super::Report {
            schema_version: 1,
            accounting_complete: true,
            selection: Selection::AllAdapters,
            adapters: reports,
        })
        .unwrap();
        let bytes = toml::to_string(&audit).unwrap();
        validate_audit_mode(bytes.as_bytes(), Mode::Current).unwrap();
        assert_eq!(audit.adapters.len(), 4);
        for target in &audit.adapters {
            assert_eq!(target.owners.len(), 11);
            assert_eq!(target.component_receipts.len(), 29);
            assert!(matches!(target.artifact, Evidence::Pending { .. }));
            assert!(matches!(target.abi, Evidence::Pending { .. }));
            assert!(!target.report.required_fit);
            for owner in &target.owners {
                assert!(matches!(owner.abi_bound, Evidence::Pending { .. }));
                assert!(!owner.allocation_owner.contains("Fixture"));
                assert!(
                    Path::new(super::super::super::ROOT)
                        .join(&owner.source_path)
                        .is_file()
                );
            }
            assert!(
                target
                    .pending_facts
                    .iter()
                    .all(|fact| !fact.next_action.is_empty())
            );
        }
    }

    #[test]
    fn capture_refuses_dirty_or_mismatched_sources_instead_of_relabeling_them() {
        for (dirty, matches) in [(true, true), (false, false), (true, false)] {
            let mut audit = fixtures::fixture();
            for target in &mut audit.adapters {
                target.report.identity.dirty = dirty;
                target.report.identity.source_tree_matches_commit = matches;
            }
            assert_eq!(
                capture(super::super::super::Report {
                    schema_version: 1,
                    accounting_complete: true,
                    selection: Selection::AllAdapters,
                    adapters: audit
                        .adapters
                        .into_iter()
                        .map(|target| target.report)
                        .collect(),
                })
                .unwrap_err(),
                Error::ResourceInput
            );
        }
    }

    #[test]
    fn capture_refuses_a_single_adapter_scope_even_with_a_complete_row_set() {
        let audit = fixtures::fixture();
        let error = capture(Report {
            schema_version: 1,
            accounting_complete: true,
            selection: Selection::Adapter {
                adapter: super::super::super::AdapterId::Fastly,
            },
            adapters: audit
                .adapters
                .into_iter()
                .map(|adapter| adapter.report)
                .collect(),
        })
        .unwrap_err();
        assert_eq!(error, Error::ResourceInput);
    }

    #[test]
    fn human_audit_names_each_term_and_all_blockers_and_does_not_promote_a_numeric_gap() {
        let input = fixtures::fixture();
        let audit = capture(super::super::super::Report {
            schema_version: 1,
            accounting_complete: true,
            selection: Selection::AllAdapters,
            adapters: input
                .adapters
                .into_iter()
                .map(|target| target.report)
                .collect(),
        })
        .unwrap();
        let output = render(&audit).unwrap();
        assert!(output.len() <= 1 << 20_usize);
        assert!(output.starts_with("# Adapter Resource Envelopes\n"));
        assert!(output.contains(&format!("```text\n{}\n```\n", audit.toolchain.trim_end())));
        assert!(output.contains("No remediation package is approved"));
        assert!(output.contains("host_native_substitute"));
        // Rendering must reflect the supplied ledger. Runtime budget tests own
        // the demand derivation; unrelated config growth must not stale this UI oracle.
        let cloudflare = &audit
            .adapters
            .iter()
            .find(|target| target.report.adapter == AdapterId::Cloudflare)
            .unwrap()
            .report;
        assert!(!cloudflare.required_fit);
        assert_eq!(
            cloudflare.validation.selected_headroom_bytes,
            Some(8_000_000)
        );
        let gap = cloudflare.validation.gap_bytes.unwrap();
        assert_eq!(
            gap,
            cloudflare
                .derived
                .total_bytes
                .checked_add(8_000_000)
                .unwrap()
                .checked_sub(128_000_000)
                .unwrap()
        );
        assert!(output.contains(&format!(
            "| cloudflare | {} | 8000000 | {gap} | blocked |",
            cloudflare.derived.total_bytes,
        )));
        let axum = &audit
            .adapters
            .iter()
            .find(|target| target.report.adapter == AdapterId::Axum)
            .unwrap()
            .report;
        assert!(!axum.required_fit);
        assert_eq!(axum.validation.selected_headroom_bytes, None);
        assert_eq!(axum.validation.gap_bytes, None);
        assert!(output.contains(&format!(
            "| axum | {} | pending | pending | blocked |",
            axum.derived.total_bytes,
        )));
        for target in &audit.adapters {
            for term in &target.owners {
                assert!(output.contains(term.term_id.0.name()));
                assert!(output.contains(&term.acquire));
                assert!(output.contains(&term.release));
            }
            for fact in &target.pending_facts {
                assert!(output.contains(&fact.fact));
                assert!(output.contains(&fact.owner));
                assert!(output.contains(&fact.next_action));
            }
        }
    }

    #[test]
    fn audit_summary_refuses_a_different_current_profile_even_when_record_arithmetic_is_valid() {
        let audit = fixtures::fixture();
        let bytes = fixtures::encode(&audit);
        let mut profiles: BTreeMap<_, _> = audit
            .adapters
            .iter()
            .map(|target| {
                (
                    target.report.adapter,
                    target.report.effective_profile_sha256.clone(),
                )
            })
            .collect();
        let output = super::super::summarize_audit(&bytes, &profiles).unwrap();
        assert!(output.starts_with(b"# Adapter Resource Envelopes\n"));
        profiles.insert(super::super::AdapterId::Spin, "0".repeat(64));
        assert_eq!(
            super::super::summarize_audit(&bytes, &profiles).unwrap_err(),
            Error::ResourceInput
        );
    }

    #[test]
    fn capture_refuses_incomplete_target_sets_and_a_dirty_nonfirst_target() {
        for remove in [None, Some(0), Some(1), Some(2), Some(3)] {
            let mut audit = fixtures::fixture();
            if let Some(index) = remove {
                audit.adapters.remove(index);
            } else {
                audit.adapters[3].report.identity.dirty = true;
            }
            assert_eq!(
                capture(super::super::super::Report {
                    schema_version: 1,
                    accounting_complete: true,
                    selection: Selection::AllAdapters,
                    adapters: audit
                        .adapters
                        .into_iter()
                        .map(|target| target.report)
                        .collect(),
                })
                .unwrap_err(),
                Error::ResourceInput
            );
        }
    }
}
