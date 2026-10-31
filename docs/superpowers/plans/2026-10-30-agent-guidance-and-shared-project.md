# Agent Guidance And Shared Project Implementation Plan

> For agentic workers: follow the approved selective adaptation of Trusted
> Server's guidance. Execute tasks in order and verify each boundary. The user
> has not authorized a commit, push, pull request or GitHub mutation.

**Goal:** Give Auctioneer a single agent-policy source, focused Claude agents
and the existing organization-wide project taxonomy.

**Architecture:** Keep root instructions short and preserve the existing crate
rules and development guide. Put all project identifiers and field mappings in
one GitHub-owned document; agent prompts link to it instead of duplicating it.
Use a regular-file CLAUDE.md import and valid agent YAML frontmatter. Do not copy
Trusted Server's dependencies, aliases or platform assumptions.

**Tech Stack:** Markdown, YAML frontmatter, Claude Code settings, GitHub CLI.

## Task 1: Inspect The Existing Contracts

- [x] Read Auctioneer's root/crate instructions, development guide and CI.
- [x] Read Trusted Server's agent policies and review workflow.
- [x] Read live project fields and organization issue types without mutations.
- [x] Verify the Claude agent-file and symlink formats against official docs.

## Task 2: Establish Canonical Guidance

**Files:** `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `.github/PROJECT.md`,
`.github/pull_request_template.md`.

- [x] Expand the root policy, preserving existing environment conventions.
- [x] Add contribution/review expectations and the canonical import wrapper.
- [x] Record existing project fields and type IDs in one reference document.
- [x] State explicit approval and no-regression rules for remote changes.

## Task 3: Add Focused Agent Roles

**Files:** `.claude/README.md`, `.claude/settings.json`,
`.claude/agents/issue-creator.md`, `.claude/agents/pr-creator.md`,
`.claude/agents/pr-reviewer.md`, `.claude/agents/build-validator.md`.

- [x] Add valid frontmatter and bounded responsibilities for each role.
- [x] Preserve review isolation, approval and commit-pinning safeguards.
- [x] Keep verification commands in the existing development guide and CI.
- [x] Add only read-only GitHub command permissions, not blanket API/write grants.

## Task 4: Verify And Hand Back

- [x] Parse JSON/YAML, resolve document links and verify the canonical import.
- [x] Compare every recorded field/type/option ID with the live read-only captures.
- [x] Confirm no duplicate project IDs outside the canonical reference.
- [x] Check installed Claude CLI validation capabilities. This installation
      supports plugin-manifest validation, not the newer standalone agent-file
      validator. Agent frontmatter was parsed independently; interactive `/agents`
      discovery and model execution were not run.
- [x] Verify only intended guidance/settings files changed; leave Rust, workflows,
      existing crate rules and immutable evidence untouched.
- [x] Leave everything uncommitted for the user's inspection.

## Verification So Far

- Parsed four agent definitions and the JSON settings; checked relative links
  and the canonical policy reference.
- Compared every field/option/type association against authenticated live
  captures: 18 fields, 15 options and seven enabled organization issue types.
  The check rejected a swapped Size-option mapping before it was corrected.
- Verified that the only 13 added permissions are read-only inspection
  commands, with no broad API or write grant and no unrelated settings change.
- Prettier check passed for the changed Markdown and JSON files. The existing
  immutable evidence and its formatting exclusions are outside this change.
- A before/after checksum inventory confirmed exactly 12 intended guidance and
  settings paths changed, with no removed files or Rust/workflow/evidence edits.
- `CLAUDE.md` references `AGENTS.md`; the installed copy passed the same
  frontmatter, settings, link and live-field association checks as staging.
- At the original guidance handoff the checkout had no first commit. That
  stage performed no GitHub writes, commit, push, pull request, deployment or
  model-executing agent session.

## Initial Import Compatibility Follow-up (2026-10-31)

After the first source commit made the application-enabled CLI check possible,
it rejected the original `CLAUDE.md` symlink with `source_identity_nonfile`.
The source inventory deliberately accepts only regular files. Replace the link
with the standard one-line `@AGENTS.md` import instead of weakening that
provenance check. This retains one policy source and is also portable to
checkouts without symlink support. The application-enabled CLI check remains
required validation of this compatibility correction.
