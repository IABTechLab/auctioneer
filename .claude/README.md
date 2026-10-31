# Claude Agent Roles

Repository policy lives in [AGENTS.md](../AGENTS.md). Root `CLAUDE.md` is a
relative symlink to that same file, not a separate policy copy. Read the
applicable crate instructions explicitly when working in a crate.

The workflow is selectively adapted from
[Trusted Server's agent guidance](https://github.com/IABTechLab/trusted-server/blob/main/AGENTS.md),
not its dependency choices, platform aliases or implementation assumptions.

| Role                                         | Responsibility                                                                               |
| -------------------------------------------- | -------------------------------------------------------------------------------------------- |
| [issue-creator](agents/issue-creator.md)     | Search, draft and, only when authorized, file an issue with the existing type/project fields |
| [pr-creator](agents/pr-creator.md)           | Inspect the actual branch, summarize verification and prepare an approved PR                 |
| [pr-reviewer](agents/pr-reviewer.md)         | Review a named revision or explicit local changes without modifying the author's branch      |
| [build-validator](agents/build-validator.md) | Choose and execute the relevant existing verification commands and report honest outcomes    |

These are Claude custom agents with YAML frontmatter, not `SKILL.md` skills.
Use `/agents` to inspect available roles in a new Claude session. Other coding
agents may read the prompts explicitly; they are not automatically registered
with those clients.

The roles link to [project policy](../.github/PROJECT.md) and the existing
[development guide](../docs/testing/development.md). Do not duplicate IDs,
toolchain versions or the full CI matrix across prompts.

`settings.json` permits selected local checks and read-only GitHub inspection.
It does not preapprove `gh api`, remote writes, deployments, commits or pushes.
An issue/PR/review role still needs explicit authorization for the actual remote
action. Do not import another checkout's `settings.local.json`, credentials,
reviews, worktrees or machine-specific plugin configuration.
