# User Sync Browser Check

This local integration check drives Chromium against the existing Axum binary
and EdgeZero-generated App. Its only extra HTTP server is a controlled publisher
fixture. No test route is added to Auctioneer.

## Run

Install Node.js 20 or later and [Playwright with Chromium](https://playwright.dev/docs/browsers).
The script resolves an installed `playwright` module, or the module directory
specified by `AUCTIONEER__BROWSER__PLAYWRIGHT_MODULE`. Missing browser dependencies fail the
check; they do not cause a skip.

```sh
cargo build -p auctioneer-adapter-axum --bin auctioneer-adapter-axum --example usersync_browser_config --features insecure-bidders --locked --offline
node scripts/check-usersync-browser.mjs
```

Run from any directory. The script locates its own worktree and Cargo target
directory. The development feature is required only for HTTP loopback origins;
normal builds continue to reject insecure configuration. Do not deploy this
test build as production qualification.

The test-only example uses the existing typed configuration, validation, and
`BlobEnvelope` builder. It does not implement another loader or integrity
protocol. Each run creates isolated temporary storage, ephemeral loopback ports,
an isolated browser context, and a child process, then cleans them up.
It supplies a public synthetic test keyring directly to the child's EdgeZero
secret store; operator credentials are never inherited. The browser stores a
signed envelope under the 4-KiB complete field ceiling. The check also deletes
the matching UID cookie and verifies an empty subsequent readback.
SIGINT, SIGTERM and SIGHUP use the same bounded shutdown path, including
interruption during browser launch. Browser shutdown closes its contexts;
there is no preceding unbounded context-close wait.
If cleanup itself fails, termination still preserves the signal exit status
and emits a fixed failure diagnostic rather than a passing result.

On a POSIX host with `pgrep`, run the shutdown regression separately:

```sh
node --test scripts/tests/usersync-browser-shutdown.test.mjs
```

Its pending-launch stub proves process/config cleanup and permission-failure
handling, not browser behavior. The permission fixture requires a non-root user.
The actual-browser command above still requires real Playwright and Chromium.

## Evidence Boundary

Axum's socket-test dev-dependency enables `insecure-bidders`, including in
an `--all-targets` test invocation. That test binary is not a secure-default
production build. Run `./scripts/check-secure-usersync.sh` to build the
production dependency graph separately and verify real socket rejection of
loopback callback configuration and absence of the test bidder. CI runs this
check explicitly; it retains the production binary before compiling tests.

The check verifies browser-generated JSON preflight, credentialed CORS, the
generated sync callback's account/bidder context through a controlled redirect,
actual Secure/SameSite=None cookie acceptance and credentialed `/getuids`
visibility, and blocked reads/preflight from a denied origin. It does not disable
browser CORS or cookie protections. A rejecting loopback proxy prevents external
browser traffic without request interception, which can synthesize preflights.
The denied preflight must actually return 403 without CORS or cookie headers;
an allowed-origin read afterward rules out an outage masquerading as denial.

All origins use the same loopback host with different ports: cross-origin, but
same-site. This is not evidence for cross-site iframe/third-party cookie
acceptance, every browser policy, a real bidder redirect, or hosted-provider
execution. Those remain separate deployment checks. Configuration failure and
recovery are covered through the actual-App `usersync_config_lifecycle` suite,
not simulated by this browser check.
