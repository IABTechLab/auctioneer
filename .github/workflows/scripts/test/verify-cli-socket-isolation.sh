#!/usr/bin/env bash
set -euo pipefail

cli_socket_script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cli_socket_helper_prefix='qualification::harness::socket_test_support::tests::'
cli_socket_real_cases=(
  qualification::harness::fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof
  qualification::harness::fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener
  qualification::harness::fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning
  qualification::harness::native::system::tests::reserve_port_releases_the_loopback_listener_before_returning
  qualification::harness::fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks
  qualification::harness::fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch
)
# Independent literals: discovery must not silently bless a deletion or rename.
cli_socket_required_helpers=(
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
)

cli_socket_refuse() {
  printf 'CLI socket cohort refused: %s\n' "$1" >&2
  return 1
}

cli_socket_lane() {
  if [[ $# -ne 1 ]]; then
    cli_socket_refuse 'expected one lane: default or workspace-insecure'
    return 1
  fi
  case "$1" in
    default) cargo_args=(--locked -p auctioneer-cli --bin auctioneer-cli) ;;
    workspace-insecure) cargo_args=(--locked --workspace --exclude auctioneer-config-data --features insecure-bidders --bin auctioneer-cli) ;;
    *) cli_socket_refuse 'unknown lane'; return 1 ;;
  esac
}

cli_socket_contains() {
  local wanted="$1" candidate
  shift
  for candidate in "$@"; do
    [[ "$candidate" != "$wanted" ]] || return 0
  done
  return 1
}

# These functions are also called by finite fixtures; keep failures explicit
# because Bash disables errexit inside functions used as conditional commands.
cli_socket_select() {
  local listing="$1" line name i
  local roster=()
  total=0
  selected=()
  selected_count=0
  helper_count=0
  filtered=0
  [[ -r "$listing" ]] || { cli_socket_refuse 'unreadable roster'; return 1; }
  while IFS= read -r line || [[ -n "$line" ]]; do
    case "$line" in
      *': test')
        name="${line%: test}"
        if ! [[ "$name" =~ ^[a-zA-Z0-9_]+(::[a-zA-Z0-9_]+)*$ ]]; then
          cli_socket_refuse 'invalid roster name'; return 1
        fi
        for ((i = 0; i < total; i++)); do
          if [[ "${roster[$i]}" == "$name" ]]; then
            cli_socket_refuse 'duplicate roster name'; return 1
          fi
        done
        roster[$total]="$name"
        total=$((total + 1))
        ;;
    esac
  done < "$listing"
  [[ "$total" -gt 0 ]] || { cli_socket_refuse 'empty roster'; return 1; }
  for name in "${cli_socket_real_cases[@]}" "${cli_socket_required_helpers[@]}"; do
    if ! cli_socket_contains "$name" "${roster[@]}"; then
      cli_socket_refuse "missing required test: $name"; return 1
    fi
  done
  selected=("${cli_socket_real_cases[@]}")
  selected_count="${#selected[@]}"
  for name in "${roster[@]}"; do
    case "$name" in
      "$cli_socket_helper_prefix"*)
        selected[$selected_count]="$name"
        selected_count=$((selected_count + 1))
        helper_count=$((helper_count + 1))
        ;;
    esac
  done
  [[ "$helper_count" -gt 0 ]] || { cli_socket_refuse 'no helper tests'; return 1; }
  filtered=$((total - selected_count))
}

cli_socket_check_result() {
  local log="$1" line name i state=before success_count=0
  local successful=()
  if [[ "$selected_count" -eq 0 || "$helper_count" -eq 0 ]]; then
    cli_socket_refuse 'empty selection'; return 1
  fi
  if ! bash "$cli_socket_script_dir/check-libtest-result.sh" "$log" "$filtered" 1 "$selected_count"; then
    cli_socket_refuse 'summary accounting'; return 1
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    case "$line" in
      running*)
        if [[ "$state" != before || "$line" != "running $selected_count tests" ]]; then
          cli_socket_refuse 'wrong or repeated run announcement'; return 1
        fi
        state=cases
        ;;
      'test result:'*)
        if [[ "$state" != cases || "$success_count" -ne "$selected_count" ]]; then
          cli_socket_refuse 'missing cases or extra report'; return 1
        fi
        state=after
        ;;
      test\ *)
        if [[ "$state" != cases || "$line" != *' ... ok' ]]; then
          cli_socket_refuse 'non-success or misplaced case'; return 1
        fi
        name="${line#test }"
        name="${name% ... ok}"
        if ! cli_socket_contains "$name" "${selected[@]}"; then
          cli_socket_refuse 'unexpected successful name'; return 1
        fi
        for ((i = 0; i < success_count; i++)); do
          if [[ "${successful[$i]}" == "$name" ]]; then
            cli_socket_refuse 'duplicate successful name'; return 1
          fi
        done
        successful[$success_count]="$name"
        success_count=$((success_count + 1))
        ;;
      *)
        # Cargo may emit build diagnostics before libtest starts, but no
        # output other than whitespace may interrupt or follow the report.
        if [[ "$line" =~ ^[[:space:]]+(running|test)[[:space:]] ]] || \
          [[ "$state" != before && "$line" == *[![:space:]]* ]]; then
          cli_socket_refuse 'unexpected report content'; return 1
        fi
        ;;
    esac
  done < "$log"
  [[ "$state" == after && "$success_count" -eq "$selected_count" ]] || {
    cli_socket_refuse 'incomplete report'; return 1;
  }
}

cli_socket_main() {
  cli_socket_lane "$@" || return 1
  local lane="$1" log_dir listing log status=0
  : "${RUNNER_TEMP:?RUNNER_TEMP is required for retained cohort logs}"
  log_dir="$(mktemp -d "$RUNNER_TEMP/cli-socket-isolation-$lane.XXXXXX")" || return 1
  listing="$log_dir/list.log"
  log="$log_dir/cohort.log"
  printf 'CLI socket cohort logs: %s\n' "$log_dir"
  cargo test "${cargo_args[@]}" -- --list --color=never 2>&1 | tee "$listing" || status=$?
  printf 'roster exit status: %s\n' "$status" | tee "$log_dir/status.log"
  [[ "$status" -eq 0 ]] || return "$status"
  cli_socket_select "$listing" || return 1
  printf 'cohort roster: total=%s selected=%s helpers=%s filtered=%s\n' \
    "$total" "$selected_count" "$helper_count" "$filtered" | tee -a "$log_dir/status.log"
  cargo test "${cargo_args[@]}" -- --exact --color=never "${selected[@]}" 2>&1 | tee "$log" || status=$?
  printf 'cohort exit status: %s\n' "$status" | tee -a "$log_dir/status.log"
  [[ "$status" -eq 0 ]] || return "$status"
  cli_socket_check_result "$log" 2>&1 | tee "$log_dir/check.log"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  cli_socket_main "$@"
fi
