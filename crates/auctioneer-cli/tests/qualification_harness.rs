#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use std::fs;
    use std::io::ErrorKind;
    use std::process::Stdio;
    use std::process::{Command as SyncCommand, Output};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
    use tokio::net::TcpStream;
    use tokio::process::Command;
    use tokio::time::timeout;

    #[tokio::test]
    async fn fixture_cli_uses_private_records_and_observes_eof_before_exiting() {
        let mut child = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "fixture"])
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        input.write_all(b"{\"kind\":\"native\",\"port\":0,\"body_cap\":4096,\"max_requests\":100,\"max_sockets\":4}\n").await.unwrap();
        let mut line = String::new();
        timeout(Duration::from_secs(5), output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let ready: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(ready["ready"], true);
        assert_eq!(ready["protocol"], 1_i32);
        let port = u16::try_from(ready["port"].as_u64().unwrap()).unwrap();
        input
            .write_all(b"{\"id\":1,\"command\":\"stats\",\"settings\":null}\n")
            .await
            .unwrap();
        line.clear();
        timeout(Duration::from_secs(5), output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], 1_i32);
        assert_eq!(reply["stats"]["calls"], 0_i32);
        drop(input);
        assert_eq!(
            timeout(Duration::from_secs(5), child.wait())
                .await
                .unwrap()
                .unwrap()
                .code(),
            Some(0_i32)
        );
        TcpStream::connect(("127.0.0.1", port)).await.unwrap_err();
    }

    #[test]
    fn provider_command_refusals_do_not_echo_input_paths_or_contents() {
        let scratch = tempfile::tempdir().unwrap();
        let input = scratch.path().join("SECRET_PATH_CANARY.json");
        fs::write(&input, b"{\"SECRET_BODY_CANARY\":true}").unwrap();
        let mut checked = 0_u8;
        for command in [
            "offline",
            "prepare",
            "capture-build",
            "stage-spin",
            "stage-fastly",
            "verify-fastly-archive",
        ] {
            let output = SyncCommand::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .args(["qualify", "providers", command, "--input"])
                .arg(&input)
                .env_clear()
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2_i32));
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert_ne!(stderr, "qualification_invalid_invocation\n");
            assert!(!stderr.contains("CANARY"));
            assert!(output.stdout.is_empty());
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 6);
        let output = SyncCommand::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args([
                "qualify",
                "providers",
                "offline",
                "--input",
                "SECRET_MISSING_PATH_CANARY",
            ])
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2_i32));
        assert_eq!(output.stdout, b"");
        assert_eq!(String::from_utf8(output.stderr).unwrap(), "invalid_input\n");
    }

    fn offline_input() -> Value {
        let template: Value = serde_json::from_str(include_str!(
            "../../../docs/testing/fixtures/provider-handlers/cases.json"
        ))
        .unwrap();
        let cases: Vec<_> = template["cases"].as_array().unwrap().iter().map(|row| {
        let kind = row["kind"].as_str().unwrap();
        let mutation = match kind {
            "malformed_config" | "digest_mismatch" | "config_replacement" => json!({
                "isolated":true,"authorized":true,"addressing_verified":true,"known_good_observed":true,
                "recovery_armed":true,"injection":"provider_write","write_visible":true,"handler_observed":true,
                "refusal":match kind {"malformed_config"=>"malformed","digest_mismatch"=>"integrity",_=>"none"},
                "restoration":"typed_push","recovered":true,"interrupted":false
            }),
            _ => Value::Null,
        };
        let bids = u8::from(matches!(kind,"banner"|"debug_denied"|"debug_redacted"|"headers"|"completion"|"config_replacement"));
        json!({"id":row["id"],"kind":kind,"transport":"http","status":row["status"].as_u64().unwrap_or(200),
            "dispatch_before":0_i32,"dispatch_after":row["dispatches"],"fixture_case_id":row["id"],"bid_count":bids,
            "failure":match kind {"bidder_failure"=>"bidder_failure","bidder_timeout"=>"bidder_timeout",_=>"none"},
            "assertions_verified":true,"mutation":mutation})
    }).collect();
        assert_eq!(cases.len(), 14);
        let digest = "a".repeat(64);
        let identity = json!({"auctioneer_commit":"a".repeat(40),"edgezero_pin":"9fe6c56c648f0a601b1c6c051ce71c09d3733ea9",
        "artifact_sha256":digest,"source_sha256":digest,"build_features":["spin","auctioneer-config/test-harness-override"],
        "target":"wasm32-wasip2","runtime_version":{"major":3_i32,"minor":0_i32,"patch":0_i32},"harness_sha256":digest,
        "fixtures_sha256":digest,"reference_data_sha256":digest,"baked_config_sha256":digest,"fixture_metadata_sha256":digest});
        json!({"schema_version":1,"kind":"offline_handler_control","provider":"spin",
        "environment":{"kind":"isolated_staging","deployment_sha256":digest,"binding_sha256":digest},
        "identity":identity,"expected_identity":identity,"cases":cases})
    }

    #[test]
    fn offline_cli_success_never_grants_deployed_acceptance_or_publisher_readiness() {
        let scratch = tempfile::tempdir().unwrap();
        let input = scratch.path().join("offline.json");
        fs::write(&input, serde_json::to_vec(&offline_input()).unwrap()).unwrap();
        let output = SyncCommand::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "providers", "offline", "--input"])
            .arg(&input)
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0_i32),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["offline_checks"], "pass");
        assert_eq!(report["deployed_handler_acceptance"], "pending");
        assert_eq!(report["resource_qualification"], "not_evaluated");
        assert_eq!(report["publisher_pilot_ready"], false);
        assert_eq!(report["cases"].as_array().unwrap().len(), 14);
    }

    fn offline_output(bytes: &[u8]) -> Output {
        let scratch = tempfile::tempdir().unwrap();
        let input = scratch.path().join("offline.json");
        fs::write(&input, bytes).unwrap();
        SyncCommand::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "providers", "offline", "--input"])
            .arg(input)
            .env_clear()
            .output()
            .unwrap()
    }

    #[test]
    fn offline_cli_rejects_duplicate_authorization_before_value_conversion() {
        let mut input = offline_input();
        input["cases"][4]["mutation"]["authorized"] = json!(false);
        let text = serde_json::to_string(&input).unwrap();
        let field = "\"authorized\":false";
        assert_eq!(text.matches(field).count(), 1);
        let denied = offline_output(text.as_bytes());
        assert_eq!(denied.status.code(), Some(2_i32));
        assert_eq!(denied.stderr, b"mutation_not_isolated\n");
        assert!(denied.stdout.is_empty());
        for duplicate in [
            "\"authorized\":false,\"authorized\":true",
            "\"authorized\":true,\"authorized\":false",
        ] {
            let ambiguous = text.replacen(field, duplicate, 1);
            let result = offline_output(ambiguous.as_bytes());
            assert_eq!(result.status.code(), Some(2_i32));
            assert_eq!(result.stderr, b"invalid_input\n");
            assert!(result.stdout.is_empty());
        }
    }

    #[test]
    fn offline_cli_rejects_duplicate_artifact_identity_before_value_conversion() {
        let mut input = offline_input();
        let bad = "b".repeat(64);
        let good = "a".repeat(64);
        input["identity"]["artifact_sha256"] = json!(bad);
        let text = serde_json::to_string(&input).unwrap();
        let field = format!("\"artifact_sha256\":\"{bad}\"");
        assert_eq!(text.matches(&field).count(), 1);
        let mismatched = offline_output(text.as_bytes());
        assert_eq!(mismatched.status.code(), Some(2_i32));
        assert_eq!(mismatched.stderr, b"identity_mismatch\n");
        assert!(mismatched.stdout.is_empty());
        let duplicate = format!("{field},\"artifact_sha256\":\"{good}\"");
        let result = offline_output(text.replacen(&field, &duplicate, 1).as_bytes());
        assert_eq!(result.status.code(), Some(2_i32));
        assert_eq!(result.stderr, b"invalid_input\n");
        assert!(result.stdout.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fixture_cli_handles_each_signal_and_closes_the_listener_before_exit() {
        use rustix::process::{Pid, Signal, kill_process};
        for (signal, expected) in [(Signal::INT, 130_i32), (Signal::TERM, 143_i32)] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .args(["qualify", "fixture"])
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let mut input = child.stdin.take().unwrap();
            input.write_all(b"{\"kind\":\"provider\",\"port\":0,\"scenario\":\"bid\",\"max_requests\":4,\"key_file\":null,\"cert_file\":null}\n").await.unwrap();
            let mut output = BufReader::new(child.stdout.take().unwrap());
            let mut line = String::new();
            timeout(Duration::from_secs(5), output.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            let ready: Value = serde_json::from_str(&line).unwrap();
            let port = u16::try_from(ready["port"].as_u64().unwrap()).unwrap();
            kill_process(
                Pid::from_raw(i32::try_from(child.id().unwrap()).unwrap()).unwrap(),
                signal,
            )
            .unwrap();
            assert_eq!(
                timeout(Duration::from_secs(5), child.wait())
                    .await
                    .unwrap()
                    .unwrap()
                    .code(),
                Some(expected)
            );
            TcpStream::connect(("127.0.0.1", port)).await.unwrap_err();
            drop(input);
        }
    }

    #[test]
    fn native_cli_rejects_a_malformed_candidate_before_build_and_does_not_echo_it() {
        let scratch = tempfile::tempdir().unwrap();
        let input = scratch.path().join("SECRET_CANDIDATE_PATH.json");
        fs::write(
            &input,
            b"{\"schema_version\":1,\"kind\":\"native_axum_candidate\",\"SECRET_BODY\":true}",
        )
        .unwrap();
        let output_dir = scratch.path().join("evidence");
        let output = SyncCommand::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "native", "--profile"])
            .arg(&input)
            .arg("--output")
            .arg(&output_dir)
            .args(["--prepare", "--no-build"])
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2_i32));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), "invalid_input\n");
        assert!(!output_dir.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_full_public_output_pipe_cannot_keep_the_runtime_alive_after_its_write_deadline() {
        use rustix::io::{Errno, write as raw_write};
        use tokio::net::unix::pipe;
        let scratch = tempfile::tempdir().unwrap();
        let input = scratch.path().join("offline.json");
        fs::write(&input, serde_json::to_vec(&offline_input()).unwrap()).unwrap();
        let (mut sender, receiver) = pipe::pipe().unwrap();
        sender.write_all(b"x").await.unwrap();
        let mut filled = 1;
        loop {
            match sender.try_write(b"x") {
                Ok(count) => {
                    filled += count;
                    assert!(filled < 16 * 1024 * 1024);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => panic!("cannot prepare bounded pipe: {error}"),
            }
        }
        assert!(filled > 0);
        assert_eq!(raw_write(&sender, b"x"), Err(Errno::AGAIN));
        let mut child = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["qualify", "providers", "offline", "--input"])
            .arg(&input)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::from(sender.into_blocking_fd().unwrap()))
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let observed = timeout(Duration::from_secs(5), child.wait()).await;
        if observed.is_err() {
            child.kill().await.unwrap();
            let _reaped = child.wait().await.unwrap();
        }
        let status = observed.unwrap().unwrap();
        assert_eq!(status.code(), Some(2_i32));
        let mut diagnostic = Vec::new();
        child
            .stderr
            .take()
            .unwrap()
            .take(1024)
            .read_to_end(&mut diagnostic)
            .await
            .unwrap();
        assert_eq!(diagnostic, b"output_limit\n");
        drop(receiver);
    }
}
