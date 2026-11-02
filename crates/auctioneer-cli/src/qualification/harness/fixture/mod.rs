use super::{HarnessError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub mod client;
pub mod protocol;
mod server;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Options {
    Native {
        port: u16,
        body_cap: usize,
        max_requests: u32,
        max_sockets: u16,
    },
    Provider {
        port: u16,
        scenario: ProviderMode,
        max_requests: u8,
        key_file: Option<PathBuf>,
        cert_file: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMode {
    Bid,
    Failure,
    Stall,
    Canary,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub id: u32,
    pub command: String,
    pub settings: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Plain,
    Gzip,
    Mixed,
    Error,
    Stall,
    Cancel,
    Park,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub mode: Mode,
    pub encoded_bytes: u64,
    pub decoded_bytes: u64,
    pub seed: u32,
    pub delay_ms: u64,
    #[serde(default)]
    pub stall_after_calls: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub calls: u32,
    pub completed: u32,
    pub fpd_expansions: u32,
    pub gzip_responses: u32,
    pub stalled_reads: u32,
    pub disconnected: u32,
    pub max_encoded_bytes: u64,
    pub max_decoded_bytes: u64,
    pub max_calls_per_auction: u32,
    pub max_active_auctions: u32,
    pub max_active_calls: u32,
    pub completed_auctions: u32,
    pub current_active_calls: u32,
    pub max_active_debug_auctions: u32,
    pub probe_first_calls: u32,
    pub probe_second_calls: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderStats {
    pub dispatches: u32,
    pub case_id: u8,
    pub rejected: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub auctions: u32,
    pub complete_auctions: u32,
}

fn request_id(id: &str) -> Result<u32> {
    let suffix = id
        .strip_prefix('q')
        .ok_or(HarnessError::new("fixture_failed"))?;
    if suffix.is_empty() || suffix.len() > 8 || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(HarnessError::new("fixture_failed"));
    }
    let numeric: u32 = suffix
        .parse()
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    if numeric.to_string() != suffix {
        return Err(HarnessError::new("fixture_failed"));
    }
    Ok(numeric)
}

fn selected(mode: Mode, id: u32, calls: u32, stall_after: u32) -> Mode {
    match mode {
        Mode::Mixed => match id.checked_rem(3) {
            Some(0) => Mode::Gzip,
            Some(1) => Mode::Error,
            _ => Mode::Stall,
        },
        Mode::Stall if id.is_multiple_of(3) || calls <= stall_after => Mode::Gzip,
        Mode::Cancel | Mode::Park => Mode::Gzip,
        other @ (Mode::Plain | Mode::Gzip | Mode::Error | Mode::Stall) => other,
    }
}

fn coverage(seen: &BTreeMap<u32, u32>, ids: &[u32], calls: u32) -> Result<Coverage> {
    if !(1..=64).contains(&calls)
        || ids.len() > 10_000
        || ids.iter().any(|id| !(1..=999_999).contains(id))
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(Coverage {
        auctions: u32::try_from(ids.len()).map_err(|_error| HarnessError::new("invalid_input"))?,
        complete_auctions: u32::try_from(
            ids.iter().filter(|id| seen.get(id) == Some(&calls)).count(),
        )
        .map_err(|_error| HarnessError::new("invalid_input"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn final_wave_stalls_do_not_remove_independent_healthy_decode_work() {
        assert!(matches!(selected(Mode::Stall, 3, 16, 14), Mode::Gzip));
        assert!(matches!(selected(Mode::Stall, 4, 14, 14), Mode::Gzip));
        assert!(matches!(selected(Mode::Stall, 4, 15, 14), Mode::Stall));
        for (id, expected) in [(3_u32, "gzip"), (4, "error"), (5, "stall")] {
            assert_eq!(
                serde_json::to_value(selected(Mode::Mixed, id, 1, 0)).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn coverage_credits_only_exact_per_auction_call_counts_and_refuses_duplicate_ids() {
        let seen = BTreeMap::from([(1_u32, 16_u32), (2, 15), (3, 17)]);
        let result = coverage(&seen, &[1, 2, 3, 4], 16).unwrap();
        assert_eq!(result.auctions, 4);
        assert_eq!(result.complete_auctions, 1);
        assert_eq!(
            coverage(&seen, &[1, 1], 16).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            coverage(&seen, &[0], 16).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            request_id("q100000000").unwrap_err(),
            HarnessError::new("fixture_failed")
        );
        assert_eq!(
            request_id("qSECRET").unwrap_err(),
            HarnessError::new("fixture_failed")
        );
        assert_eq!(
            request_id("q01").unwrap_err(),
            HarnessError::new("fixture_failed")
        );
        assert_eq!(request_id("q99999999").unwrap(), 99_999_999);
    }

    #[tokio::test]
    async fn provider_listener_returns_real_bids_and_numeric_control_snapshots() {
        use super::super::http::{Category, RequestSpec, request};
        use super::super::process::Supervisor;
        use bytes::Bytes;
        use http::Method;
        let mut server = server::Server::start(Options::Provider {
            port: 0,
            scenario: ProviderMode::Bid,
            max_requests: 2,
            key_file: None,
            cert_file: None,
        })
        .await
        .unwrap();
        let request_body =
            serde_json::to_vec(&json!({"id":"handler-case-1","imp":[{"id":"fixture-imp"}]}))
                .unwrap();
        let response_owner = Supervisor::new();
        let response_result = request(
            &format!("http://127.0.0.1:{}/openrtb2/auction", server.port),
            RequestSpec {
                method: Method::POST,
                body: Bytes::from(request_body),
                inspect: true,
                ..RequestSpec::default()
            },
            &response_owner,
        )
        .await;
        response_owner.cleanup().await.unwrap();
        let response = response_result.unwrap();
        assert_eq!(response.category, Category::Success);
        let bid: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(bid["seatbid"][0]["bid"][0]["price"], 1.25_f64);
        let snapshot = server
            .command(Control {
                id: 1,
                command: "snapshot".to_owned(),
                settings: Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(
            snapshot,
            json!({"id":1_u32,"dispatches":1_u32,"case_id":1_u8,"rejected":0_u32})
        );
        let refusal_owner = Supervisor::new();
        let refusal_result = request(
            &format!("http://127.0.0.1:{}/snapshot", server.port),
            RequestSpec::default(),
            &refusal_owner,
        )
        .await;
        refusal_owner.cleanup().await.unwrap();
        let refused = refusal_result.unwrap();
        assert_eq!(refused.status, 404);
        server.stop().await.unwrap();
    }
}
