# Workflow Shell Scripts

Substantive shell steps live beside their owning workflow under
`scripts/<workflow-stem>/`. Invoke them explicitly with Bash from the repository
root, for example:

```sh
bash .github/workflows/scripts/test/execute-catalog-publication.sh
```

All workflow invocations require the repository root as their working directory.
The `test/`, `ci-fastly/`, `ci-spin/`, and `refresh-gvl/` scripts also depend on
root-relative manifests, artifacts, `.tool-versions`, or Git operations. The
`ci-cloudflare/measure-upload.sh` body only reads `/tmp/wrangler.log`, but its
workflow invocation follows the same root-relative convention. Scripts do not
change directories implicitly.

These are CI steps, not independent setup commands. Their owning YAML retains
toolchain setup, dependency fetches, environment bindings, conditions, timeouts,
step IDs, and ordering. Scripts inherit that environment and write the same
`GITHUB_OUTPUT`, `GITHUB_ENV`, `GITHUB_PATH`, and `GITHUB_STEP_SUMMARY` files as
their former inline bodies. Downloads, target builds, and artifact measurements
require the tools and preceding steps from that workflow.

The refresh verdict script discards a no-op refresh with `git checkout -- .`.
The pull-request script creates commits, pushes a branch, and creates or updates
a pull request. Run these only in the disposable refresh checkout with its
workflow-provided environment and credentials.

Review workflow configuration directly. Validate script behavior by executing
the appropriate script or tool; do not duplicate YAML or shell bodies in Rust
configuration-snapshot tests. The actual build, test and resource checks remain
in the workflows and scripts.
