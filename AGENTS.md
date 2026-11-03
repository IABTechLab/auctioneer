# Auctioneer Agent Guidance

This is the canonical policy for all coding agents. `CLAUDE.md` imports this
file with `@AGENTS.md`; edit this file, not a duplicate. Keep that import as a
regular file because the CLI's source-identity contract rejects symlinks. Read
[crate rules](crates/AGENTS.md) before changing any crate. More-specific rules
add constraints; they must not weaken those rules.

## Project And Boundaries

- Repository: `IABTechLab/auctioneer`. Auctioneer is an OpenRTB 2.6 auction
  engine and embeddable library built on EdgeZero.
- Keep HTTP/runtime integration in the adapter crates. Shared auction,
  bidder, privacy and configuration logic must remain portable.
- Axum is native; Cloudflare uses `wasm32-unknown-unknown`, Fastly uses
  `wasm32-wasip1`, and Spin uses `wasm32-wasip2`. Native success does not prove
  guest execution, provider readiness or memory fit.
- Use `.tool-versions`, root workspace dependency/metadata declarations and
  `Cargo.lock` as their respective sources of truth. Do not copy tool versions
  or dependency pins into agent prompts.
- Follow existing error types and library choices. Do not migrate error
  frameworks or import Trusted Server-specific architecture as a style change.

## Working Agreement

- Read the relevant implementation, specification, plan and existing tests
  before changing a contract. Check current code instead of relying on an old
  review or capability description.
- For substantial work, agree on the scope and design before implementation.
  Keep edits focused; record unresolved requirements rather than silently
  weakening acceptance criteria.
- Preserve the user's changes, worktrees and local configuration. Do not
  reset, clean or overwrite work you did not create. Never run mutations or
  experiments in another reviewer's working tree.
- A local change request is not authorization to publish. Obtain approval
  before remote writes, deployments, force-pushes or public review submission.
  An explicit hold on commits, pushes or PRs remains in effect until lifted.
- Never copy credentials, customer data, private logs or local settings into
  source, fixtures, issues, PRs or agent prompts. Use fictional examples.
  Legitimate provider endpoints, source provenance and attribution stay with
  their real owner; do not replace production protocol data with example URLs.

## Implementation And Tests

- Inherit shared dependency paths/versions, package metadata and lints from
  the workspace. Keep crate-specific names, descriptions, features and targets
  local. Report incompatible provider tooling instead of duplicating metadata
  or claiming that Cargo validation proves provider packaging.
- Prefer typed boundaries, borrowed inputs and existing helpers. Do not add
  abstractions, dependencies, public APIs or clones without a concrete need.
- Fail closed on privacy, authorization and untrusted-input errors. Do not
  log consent, identifiers, credentials or rejected raw configuration values.
  Use the existing logging system; stdout remains available for intended CLI
  output, not application diagnostics.
- Keep unit tests inline beside their implementation. Conformance and
  integration tests stay with their owner. Shared infrastructure uses capable
  fake bidders, not a real bidder as a stand-in. Preserve independent oracles
  and process/allocator isolation.
- Use `serde_json::json!` for valid JSON fixtures. Keep only justified raw
  lexical/error controls, within the existing fixture gate. Do not introduce
  handwritten source imports or Rust string-based code generation.
- Test the failing branch and adjacent boundaries. Vary independent inputs
  independently, assert before-state presence when testing removal, and check
  that filtered tests actually execute. Do not replace an independent expected
  value with the same constant or implementation being tested.
- Keep `.expect()` test-only under the existing lint policy; handle fallible
  production inputs explicitly. Document public contracts, errors and relevant
  resource ownership without adding boilerplate to untouched code.

## Verification

Use [development and test ownership](docs/testing/development.md) for focused
commands and prerequisites. The workflows in `.github/workflows/` define the
full acceptance matrix; do not copy it into multiple agent files.

- Start with the affected package/module, then run the impacted public and
  adapter contracts. Check `required-features` and actual dispatch counts.
- Serialize heavy Cargo jobs and retain incremental caches. Do not routinely
  clean targets or disable incremental builds to prepare a test run.
- Distinguish compilation, executed tests, allocation measurements, hosted
  checks and complete qualification. Report exact commands and pass/fail/
  not-run/blocked results. Missing prerequisites and jobs that never ran are
  not passing verification.
- Do not lower workload limits, raise ceilings, change frozen thresholds,
  remove watches, rewrite expected captures or weaken tests to obtain green.
- Inspect CI's actual steps and results separately from local checks. Keep
  substantial workflow shell scripts beside their workflow; do not recreate
  Rust snapshots of workflow YAML or shell bodies.

## Environment Variables

- Auctioneer-owned environment variables use `__` between namespace, section and
  field components. Single underscores stay inside a component: for example,
  `AUCTIONEER__BIDDER__SOURCE_DIR` and
  `AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN`.
- Apply the same convention to test controls, CLI tooling, CI, scripts and
  current documentation. Update readers and callers together when renaming;
  do not add legacy aliases without explicit approval.
- Preserve externally defined environment names such as `PATH`,
  `CARGO_TARGET_DIR` and SDK-owned integration variables.
- Preserve original bytes in historical evidence and external captures. Their
  recorded names are data, not active environment-variable interfaces.

## Issues, Reviews And Shipping

- Follow [contribution guidance](CONTRIBUTING.md) and
  [shared project policy](.github/PROJECT.md). Reuse the organization types and
  existing project fields; do not invent parallel taxonomies.
- Search for existing issues before creating one. Substantial changes need a
  clear issue, acceptance criteria and the appropriate reviewed design. Do not
  create an issue merely to decorate a small PR.
- Reviews report concrete correctness, security, privacy, resource and
  regression findings with file/line references. Verify unfamiliar behavior
  before flagging it; do not treat formatter preferences as defects.
- Reviewers do not commit fixes, push branches, merge, change project status
  or post a GitHub review without the corresponding explicit request.
- Use concise, imperative, sentence-case commit/PR titles. Do not add semantic
  prefixes, generated footers, bylines or `Co-Authored-By` trailers.
- Claude's [focused roles](.claude/README.md) share this policy. Other agents
  can read those role prompts explicitly; they do not replace repository rules.
