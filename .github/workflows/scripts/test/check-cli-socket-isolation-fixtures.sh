#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "$script_dir/verify-cli-socket-isolation.sh"
: "${RUNNER_TEMP:?RUNNER_TEMP is required for finite checker fixtures}"
scratch="$(mktemp -d "$RUNNER_TEMP/cli-socket-isolation-fixtures.XXXXXX")"
trap 'status=$?; if [[ "$status" -eq 0 ]]; then rm -rf -- "$scratch"; else printf "CLI socket fixture files retained: %s\n" "$scratch" >&2; fi' EXIT
checked=0

# Independent fixture roster, including a future helper not in the required
# list. Two unrelated tests must contribute to filtered, not selected.
fixture_cases=(
  qualification::harness::fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof
  qualification::harness::fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener
  qualification::harness::fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning
  qualification::harness::native::system::tests::reserve_port_releases_the_loopback_listener_before_returning
  qualification::harness::fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks
  qualification::harness::fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch
  qualification::harness::socket_test_support::tests::closed_case_roster_matches_six_owned_tests
  qualification::harness::socket_test_support::tests::report_acceptance_requires_one_exact_success
  qualification::harness::socket_test_support::tests::child_role_rejects_invalid_controls_before_body
  qualification::harness::socket_test_support::tests::parent_delegation_defers_body_and_child_runs_it_once
  qualification::harness::socket_test_support::tests::owned_wrappers_delegate_before_constructing_resources
  qualification::harness::socket_test_support::tests::wrapper_guard_rejects_missing_mismatched_and_disabled_cases
  qualification::harness::socket_test_support::tests::zero_test_child_success_is_refused
  qualification::harness::socket_test_support::tests::child_assertion_failure_preserves_only_bounded_stderr
  qualification::harness::socket_test_support::tests::post_spawn_setup_failure_retains_owner_until_reap
  qualification::harness::socket_test_support::tests::timeout_refuses_and_observes_reap
  qualification::harness::socket_test_support::tests::stdout_and_stderr_overflow_are_independent_refusals
  qualification::harness::socket_test_support::tests::capture_reads_pin_cap_minus_one_cap_and_plus_one
  qualification::harness::socket_test_support::tests::helper_policy_defaults_match_independent_literals
  qualification::harness::socket_test_support::tests::contract_controls_reject_passthrough_empty_success_and_accept_all
  qualification::harness::socket_test_support::tests::cleanup_polls_after_kill_error
  qualification::harness::socket_test_support::tests::cleanup_fence_failure_overrides_original_refusal
  qualification::harness::socket_test_support::tests::capture_removal_failure_is_reported
  qualification::harness::socket_test_support::tests::report_control_roster_is_complete
  qualification::harness::socket_test_support::tests::witness_only_candidate_is_refused
  qualification::harness::socket_test_support::tests::additional_fixture_control
)

check() {
  local expected="$1" label="$2" status=0
  shift 2
  ( "$@" ) > "$scratch/decision.log" 2>&1 || status=$?
  if [[ "$expected" == pass && "$status" -ne 0 ]] || [[ "$expected" == refuse && "$status" -eq 0 ]]; then
    printf 'CLI socket fixture decision mismatch: %s (%s)\n' "$label" "$status" >&2
    exit 1
  fi
  checked=$((checked + 1))
}

check_lane() {
  local lane="$1" expected="$2"
  cli_socket_lane "$lane" || return 1
  [[ "${cargo_args[*]}" == "$expected" ]]
}

check_cohort() {
  cli_socket_select "$1" || return 1
  [[ "$total" -eq 28 && "$selected_count" -eq 26 && "$helper_count" -eq 20 && "$filtered" -eq 2 ]] || return 1
  cli_socket_check_result "$2"
}

check_minimum_cohort() {
  cli_socket_select "$1" || return 1
  [[ "$total" -eq 27 && "$selected_count" -eq 25 && "$helper_count" -eq 19 && "$filtered" -eq 2 ]] || return 1
  cli_socket_check_result "$2"
}

printf '%s: test\n' "${fixture_cases[@]}" tests::unrelated tests::unrelated_ignored > "$scratch/valid.list"
printf '%s\n' 'tests::unrelated_benchmark: benchmark' >> "$scratch/valid.list"
{
  printf '    Finished `test` profile\n     Running CLI unit tests\n\nrunning 26 tests\n'
  # Reverse discovery order: completion order is not a serialization promise.
  for ((i = ${#fixture_cases[@]} - 1; i >= 0; i--)); do
    printf 'test %s ... ok\n' "${fixture_cases[$i]}"
  done
  printf '\ntest result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.01s\n\n'
} > "$scratch/valid.log"

check pass default-lane check_lane default '--locked -p auctioneer-cli --bin auctioneer-cli'
check pass insecure-lane check_lane workspace-insecure '--locked --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli'
check refuse unknown-lane cli_socket_lane unknown
check refuse missing-lane cli_socket_lane
check refuse extra-lane cli_socket_lane default workspace-insecure
check pass valid-cohort check_cohort "$scratch/valid.list" "$scratch/valid.log"
printf '%s: test\n' "${fixture_cases[@]:0:25}" tests::unrelated tests::unrelated_ignored > "$scratch/minimum.list"
{
  printf 'running 25 tests\n'
  printf 'test %s ... ok\n' "${fixture_cases[@]:0:25}"
  printf 'test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 1s\n'
} > "$scratch/minimum.log"
check pass minimum-cohort check_minimum_cohort "$scratch/minimum.list" "$scratch/minimum.log"

# Remove and rename each mandatory member independently, not just one example.
for ((i = 0; i < 25; i++)); do
  name="${fixture_cases[$i]}"
  : > "$scratch/missing.list"
  : > "$scratch/renamed.list"
  for candidate in "${fixture_cases[@]}" tests::unrelated tests::unrelated_ignored; do
    if [[ "$candidate" == "$name" ]]; then
      printf '%s_renamed: test\n' "$candidate" >> "$scratch/renamed.list"
    else
      printf '%s: test\n' "$candidate" >> "$scratch/missing.list"
      printf '%s: test\n' "$candidate" >> "$scratch/renamed.list"
    fi
  done
  check refuse "missing-member-$i" cli_socket_select "$scratch/missing.list"
  check refuse "renamed-member-$i" cli_socket_select "$scratch/renamed.list"
done

for name in "${fixture_cases[0]}" "${fixture_cases[6]}" tests::unrelated; do
  { printf '%s: test\n' "$name"; printf '%s\n' "$(< "$scratch/valid.list")"; } > "$scratch/duplicate.list"
  check refuse duplicate-roster cli_socket_select "$scratch/duplicate.list"
done
: > "$scratch/empty.list"
check refuse empty-roster cli_socket_select "$scratch/empty.list"
printf '%s: test\n' tests::unrelated > "$scratch/zero-selected.list"
check refuse zero-selections cli_socket_select "$scratch/zero-selected.list"
printf '%s: test\n' "${fixture_cases[@]:0:6}" > "$scratch/no-helpers.list"
check refuse zero-helpers cli_socket_select "$scratch/no-helpers.list"
printf '%s: test\n' "${fixture_cases[@]:6}" > "$scratch/no-real.list"
check refuse zero-real-cases cli_socket_select "$scratch/no-real.list"

valid="$(< "$scratch/valid.log")"
case_line="test ${fixture_cases[0]} ... ok"
summary='test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.01s'

report_control() {
  local label="$1" body="$2"
  printf '%s\n' "$body" > "$scratch/report.log"
  check refuse "$label" check_cohort "$scratch/valid.list" "$scratch/report.log"
}

report_control ignored-case "${valid/$case_line/test ${fixture_cases[0]} ... ignored}"
report_control failed-case "${valid/$case_line/test ${fixture_cases[0]} ... FAILED}"
report_control ignored-summary "${valid/0 ignored/1 ignored}"
report_control failed-summary "${valid/0 failed/1 failed}"
report_control measured-summary "${valid/0 measured/1 measured}"
report_control failed-status "${valid/test result: ok./test result: FAILED.}"
report_control wrong-passed "${valid/26 passed/25 passed}"
report_control zero-passed "${valid/26 passed/0 passed}"
report_control wrong-filtered "${valid/2 filtered out/3 filtered out}"
report_control wrong-running "${valid/running 26 tests/running 25 tests}"
report_control zero-running "${valid/running 26 tests/running 0 tests}"
report_control missing-running "${valid/running 26 tests/}"
report_control duplicate-running "${valid/running 26 tests/$'running 26 tests\nrunning 26 tests'}"
report_control missing-success "${valid/$case_line/}"
report_control wrong-success "${valid/$case_line/test tests::unselected ... ok}"
report_control duplicate-success "${valid/$case_line/test ${fixture_cases[1]} ... ok}"
report_control extra-success "${valid/$summary/test tests::unselected ... ok$'\n'$summary}"
report_control truncated-success "${valid/$case_line/test ${fixture_cases[0]} ... o}"
report_control invalid-success "${valid/$case_line/$case_line trailing}"
report_control missing-additional-helper "${valid/test ${fixture_cases[25]} ... ok/}"
report_control missing-summary "${valid/$summary/}"
report_control duplicate-summary "$valid"$'\n'"$summary"
report_control nested-report "$valid"$'\n'"$valid"
report_control trailing-case "$valid"$'\n'"$case_line"
report_control trailing-run "$valid"$'\n''running 26 tests'
report_control success-before-run "$case_line"$'\n'"$valid"
report_control summary-before-run "$summary"$'\n'"$valid"
report_control indented-nested-run $'  running 1 test\n'"$valid"
report_control indented-nested-success "  $case_line"$'\n'"$valid"
report_control truncated-report "${valid%test result:*}test result: ok. 26 passed;"
report_control invalid-duration "${valid/0.01s/unknown}"
report_control invalid-report "${valid/$summary/test result: invalid}"
report_control extra-content "$valid"$'\n''unexpected trailing content'
report_control empty-report ''

# The full CLI owner has other targets and unrelated cases. Its additional
# dispatch guard must still require every owned name, without another Cargo run.
{
  printf '    Finished `test` profile\n     Running CLI unit tests\n\nrunning 28 tests\n'
  printf 'test %s ... ok\n' "${fixture_cases[@]}" tests::unrelated
  printf 'test tests::unrelated_ignored ... ignored\n'
  printf 'test result: ok. 27 passed; 0 failed; 1 ignored; 0 measured; 2 filtered out; finished in 1s\n'
  printf '     Running another integration target\nrunning 1 test\n'
  printf 'test tests::another_target ... ok\n'
  printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
} > "$scratch/full.log"
check pass full-cli-execution cli_socket_check_execution "$scratch/full.log"
check pass minimum-cli-execution cli_socket_check_execution "$scratch/minimum.log"
check refuse missing-cli-log cli_socket_check_execution "$scratch/does-not-exist.log"
check refuse empty-cli-log cli_socket_check_execution "$scratch/empty.list"
full="$(< "$scratch/full.log")"

full_control() {
  local label="$1" body="$2"
  printf '%s\n' "$body" > "$scratch/full-control.log"
  check refuse "$label" cli_socket_check_execution "$scratch/full-control.log"
}

for ((i = 0; i < 25; i++)); do
  name="${fixture_cases[$i]}"
  case_line="test $name ... ok"
  full_control "full-missing-$i" "${full/$case_line/}"
  full_control "full-renamed-$i" "${full/$case_line/test ${name}_renamed ... ok}"
  full_control "full-ignored-$i" "${full/$case_line/test $name ... ignored}"
  full_control "full-failed-$i" "${full/$case_line/test $name ... FAILED}"
  full_control "full-duplicate-$i" "$full"$'\n'"$case_line"
done
case_line="test ${fixture_cases[25]} ... ok"
full_control full-extra-helper-ignored "${full/$case_line/test ${fixture_cases[25]} ... ignored}"
full_control full-extra-helper-failed "${full/$case_line/test ${fixture_cases[25]} ... FAILED}"
full_control full-extra-helper-duplicate "$full"$'\n'"$case_line"

# Never echo synthetic libtest reports into the workflow's result accounting.
printf 'CLI socket checker fixtures: %s decisions matched\n' "$checked"
