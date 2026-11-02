# Shared GitHub Project Policy

This is the single local reference for project IDs, field/option IDs and issue
types. Other agent/contributor instructions link here; do not copy these
mappings into prompts, scripts or templates. Recheck live values before writes.

## Repository And Board

| Setting             | Value                                                      |
| ------------------- | ---------------------------------------------------------- |
| Issue/PR repository | `IABTechLab/auctioneer`                                    |
| Project owner       | `IABTechLab`                                               |
| Project number      | `3`                                                        |
| Project title       | Trusted Server Development                                 |
| Project URL         | https://github.com/orgs/IABTechLab/projects/3              |
| Project node ID     | `PVT_kwDOBPEB8s4BFKrl`                                     |
| Visibility          | Private                                                    |
| Last verified       | 2026-10-30, using authenticated read-only GitHub CLI calls |

Auctioneer and Trusted Server share this board. Keep Auctioneer issues in the
Auctioneer repository and use the existing Repository field to distinguish
them. Do not create an Auctioneer-only duplicate board, project fields or
parallel taxonomy. A shared parent initiative/epic can link repository-owned
work when approved; do not duplicate it automatically.

## Existing Fields

Built-in issue/PR data remain owned by GitHub/the repository, not copied into
new custom fields. Milestones are repository-scoped: use an approved Auctioneer
milestone rather than trying to attach a Trusted Server milestone directly.
The issue Type is an organization issue property, not a substitute label or
new project single-select field.

| Field                | Existing Field ID                | Treatment                                            |
| -------------------- | -------------------------------- | ---------------------------------------------------- |
| Title                | `PVTF_lADOBPEB8s4BFKrlzg2lUq4`   | Issue/PR title                                       |
| Assignees            | `PVTF_lADOBPEB8s4BFKrlzg2lUq8`   | Approved ownership; do not guess                     |
| Status               | `PVTSSF_lADOBPEB8s4BFKrlzg2lUrA` | Existing options below                               |
| Labels               | `PVTF_lADOBPEB8s4BFKrlzg2lUrE`   | Supplemental tags only; not duplicate types          |
| Linked pull requests | `PVTF_lADOBPEB8s4BFKrlzg2lUrI`   | Real linked work, not a copied URL field             |
| Milestone            | `PVTF_lADOBPEB8s4BFKrlzg2lUrM`   | Approved repository milestone                        |
| Repository           | `PVTF_lADOBPEB8s4BFKrlzg2lUrQ`   | Built-in repository identity                         |
| Reviewers            | `PVTF_lADOBPEB8s4BFKrlzg2lUrY`   | Actual requested reviewers                           |
| Parent issue         | `PVTF_lADOBPEB8s4BFKrlzg2lUrc`   | Approved real hierarchy                              |
| Sub-issues progress  | `PVTF_lADOBPEB8s4BFKrlzg2lUrg`   | Derived hierarchy progress                           |
| Priority             | `PVTSSF_lADOBPEB8s4BFKrlzg22nQg` | Existing options below                               |
| Size                 | `PVTSSF_lADOBPEB8s4BFKrlzg22m2Q` | Existing options below                               |
| Estimate             | `PVTF_lADOBPEB8s4BFKrlzg22mhU`   | Approved numeric estimate in the team's agreed units |
| Start Date           | `PVTF_lADOBPEB8s4BFKrlzg22nb0`   | Approved date; do not invent                         |
| End Date             | `PVTF_lADOBPEB8s4BFKrlzg22nc0`   | Approved date; do not invent                         |
| Created              | `PVTF_lADOBPEB8s4BFKrlzhN216g`   | GitHub-derived timestamp                             |
| Updated              | `PVTF_lADOBPEB8s4BFKrlzhN216s`   | GitHub-derived timestamp                             |
| Closed               | `PVTF_lADOBPEB8s4BFKrlzhN2168`   | GitHub-derived timestamp                             |

### Status Options

| Value       | Existing Option ID |
| ----------- | ------------------ |
| Backlog     | `8b41a45a`         |
| Ready       | `f75ad846`         |
| In progress | `47fc9ee4`         |
| In review   | `4424127f`         |
| Done        | `98236657`         |
| Won't Fix   | `b622b030`         |

### Priority Options

| Value | Existing Option ID |
| ----- | ------------------ |
| P0    | `c92c6358`         |
| P1    | `dca9fbdd`         |
| P2    | `afab7bc8`         |
| P3    | `71861fe7`         |

### Size Options

| Value | Existing Option ID |
| ----- | ------------------ |
| XS    | `771e90ab`         |
| S     | `01b13689`         |
| M     | `084dacdb`         |
| L     | `762f39e7`         |
| XL    | `27016a48`         |

Reuse the team's meanings, not a new Auctioneer scale. Do not translate Size
to hours, select a default Priority/Size, or calculate Estimate from either
without an approved team convention. Keep unknown values blank and report
them. Preserve existing values unless a change is explicitly authorized.

## Existing Organization Issue Types

These are enabled IABTechLab issue types. Use the actual type property, not
`bug`, `task`, `story` or similar labels as a replacement.

| Type       | Existing Node ID      | Typical Use                                                |
| ---------- | --------------------- | ---------------------------------------------------------- |
| Task       | `IT_kwDOBPEB8s4A35x7` | Scoped implementation, maintenance, CI or refactoring work |
| Bug        | `IT_kwDOBPEB8s4A35x9` | Unexpected behavior with a concrete reproducer             |
| Feature    | `IT_kwDOBPEB8s4A35yA` | New functionality                                          |
| Epic       | `IT_kwDOBPEB8s4BweiG` | Coordinated work across several issues                     |
| Initiative | `IT_kwDOBPEB8s4BweiH` | Broader product or technical objective                     |
| Story      | `IT_kwDOBPEB8s4BweiI` | User-facing capability and acceptance criteria             |
| Feedback   | `IT_kwDOBPEB8s4B4iHY` | Feedback requiring triage                                  |

## Write And Lifecycle Rules

1. Inspect/search first using the explicit repository. Check open and closed
   issues, existing project membership, current status and hierarchy.
2. Obtain authorization for remote changes. An approved issue-creation action
   under this policy includes its approved type, membership in this board and
   Backlog status unless a different initial status was approved. It does not
   authorize guessed priority, size, estimate, assignment, dates or milestones.
3. Re-read the live project, field/options and enabled type IDs before a write.
   Treat this document as a reviewed lookup, not proof that values never change.
   If names/IDs disagree or access is missing, stop and report the mismatch;
   do not create replacement fields/types or broaden permissions silently.
4. Find the issue's item on this exact project. Paginate responses; do not
   assume the first project item belongs here, or add the same issue twice.
   Use structured JSON payloads for GraphQL/REST mutations, not shell-expanded
   issue bodies. Read back applied values and report partial failures.
5. Set Backlog explicitly for approved new items; do not assume a server-side
   default. Ready means accepted for implementation, In progress means active
   implementation, and In review means the relevant review has started.
   Request/verify those transitions instead of deriving them solely from the
   existence of a branch or PR.
6. Do not regress In review/Done to In progress when opening or editing a PR.
   Reopening or intentionally moving backward requires approval and rationale.
7. Done means the issue's acceptance criteria are met, not simply that one PR
   was opened or merged. Do not close an epic/initiative because one child
   completed. Won't Fix requires an explicit maintainer decision and rationale.
8. Keep a single primary project item for the issue where one exists. A linked
   PR supplies implementation context; don't automatically add duplicate PR
   cards or overwrite other projects' field values.
9. Creating/renaming fields, options, types, views, workflows or shared-board
   settings is outside routine issue/PR handling and needs separate approval.

## Read-Only Discovery

Run from an authenticated environment with appropriate project access:

```sh
gh project view 3 --owner IABTechLab --format json
gh project field-list 3 --owner IABTechLab --limit 100 --format json
gh api orgs/IABTechLab/issue-types
gh issue list --repo IABTechLab/auctioneer --state all --search "<topic>"
```

The field-list output and REST issue-type response are the current read sources.
Do not persist unrelated private board items, tokens or customer issue content
in this repository. These read-only commands do not authorize a later mutation.
