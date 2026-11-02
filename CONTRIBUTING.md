# Contributing

Auctioneer accepts focused contributions under Apache-2.0. Read
[AGENTS.md](AGENTS.md), [crate conventions](crates/AGENTS.md) and
[development and test ownership](docs/testing/development.md) before starting.

## Scope And Design

Search `IABTechLab/auctioneer` issues before opening another one. Features and
substantial changes need an issue describing the problem, intended behavior,
affected adapters, privacy/resource effects and acceptance criteria. Small
fixes link an existing issue when available; they do not require an artificial
tracking issue.

New behavior and public/configuration contract changes need a reviewed design
under `docs/superpowers/specs/` and an implementation plan where appropriate.
Review claims about existing behavior against actual code. Record review
findings, their resolution and remaining limits; agent review does not replace
maintainer approval. Documentation-only changes and behavior-preserving
refactors need no separate spec unless their scope or risk warrants one.

Keep one PR focused on one outcome. Do not reformat unrelated code or broaden
dependencies and public APIs merely to make implementation easier. Keep
bidder-specific protocols and fixtures with the bidder; shared behavior uses
synthetic test inputs and the shared fake.

## Tracking Work

Issues belong to this repository and use the existing organization-wide board.
The [shared project policy](.github/PROJECT.md) is the only local reference for
its IDs, fields, issue types and status transitions. Preserve another project's
items and metadata. Ask before changing shared fields or workflows.

Use fictional examples and sanitized test data. Do not publish credentials,
customer data, raw consent/identifiers or private vulnerability details in an
issue, PR, review or verification transcript.

## Verification And Review

Run the smallest relevant loop first, then affected integration, conformance
and adapter checks. Use the development guide for commands and the actual
workflows for the complete matrix. Native checks are not proof of guest
execution, resource fit or a successful deployment.

In the PR, link the relevant issue/design and report exact commands, executed
test counts and unresolved failures. Clearly label checks that were blocked
or not run. A CI job that never started is not a passing check.

Review the actual commit and base, check the full affected code paths and
support findings with reproducible evidence. Draft findings locally first;
publishing a review, applying fixes and updating project state are separate
actions. Do not silently modify an author's branch while reviewing it.

## Commits And Publication

Use a short, imperative, sentence-case subject without semantic prefixes or
bracketed tags. Do not add generated footers, agent bylines or co-author
trailers. Keep source, documentation, license notices and approved changes
together without unrelated metadata churn.

Do not commit, push, create a PR or deploy while the user has placed that
action on hold. Changing a branch's scope or completing local verification
does not remove the hold.
