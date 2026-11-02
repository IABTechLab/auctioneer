use super::super::{
    HarnessError, Result,
    process::{ManagedProcess, ProcessSpec, Supervisor},
};
use super::{Control, Coverage, Options, Settings, Stats, coverage};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    #[serde(rename = "ready")]
    active: bool,
    protocol: u8,
    port: u16,
}

fn ready_port(value: Ready, expected: u16) -> Result<u16> {
    if !value.active
        || value.protocol != 1
        || value.port == 0
        || (expected != 0 && value.port != expected)
    {
        return Err(HarnessError::new("invalid_child_output"));
    }
    Ok(value.port)
}

pub struct Client {
    sequence: u32,
    process: ManagedProcess,
    supervisor: Supervisor,
}

pub trait FixtureControl {
    fn configure(&mut self, settings: &Settings) -> impl Future<Output = Result<Stats>> + Send;
    fn stats(&mut self) -> impl Future<Output = Result<Stats>> + Send;
    fn probe(&mut self, first: u32, second: u32) -> impl Future<Output = Result<Stats>> + Send;
    fn release(&mut self, all: bool) -> impl Future<Output = Result<Stats>> + Send;
    fn coverage(
        &mut self,
        ids: &[u32],
        calls: u32,
    ) -> impl Future<Output = Result<Coverage>> + Send;
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatsReply {
    id: u32,
    stats: Stats,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageReply {
    id: u32,
    coverage: Coverage,
}

trait Reply {
    fn sequence(&self) -> u32;
}
impl Reply for StatsReply {
    fn sequence(&self) -> u32 {
        self.id
    }
}
impl Reply for CoverageReply {
    fn sequence(&self) -> u32 {
        self.id
    }
}

impl Client {
    pub async fn start(binary: &Path, options: &Options, supervisor: &Supervisor) -> Result<Self> {
        let mut spec = ProcessSpec::new(binary);
        spec.args = ["qualify".into(), "fixture".into()].into();
        spec.stdin = true;
        spec.output_cap = 4 << 20_u32;
        spec.timeout_ms = 1_200_000;
        let process = supervisor.spawn(spec).await?;
        let ready = async {
            process.write_json(options).await?;
            let expected = match options {
                Options::Native { port, .. } | Options::Provider { port, .. } => *port,
            };
            ready_port(process.read_json_line().await?, expected)
        }
        .await;
        match ready {
            Ok(_port) => Ok(Self {
                sequence: 0,
                process,
                supervisor: supervisor.clone(),
            }),
            Err(error) => {
                supervisor.stop(&process).await?;
                Err(error)
            }
        }
    }

    pub async fn assert_running(&self) -> Result<()> {
        self.supervisor.check_active().await?;
        self.process.assert_running().await
    }

    async fn command<T: DeserializeOwned + Reply>(
        &mut self,
        command: &str,
        settings: Value,
    ) -> Result<T> {
        self.assert_running().await?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .filter(|value| *value <= 1_000_000)
            .ok_or(HarnessError::new("fixture_failed"))?;
        self.process
            .write_json(&Control {
                id: self.sequence,
                command: command.to_owned(),
                settings,
            })
            .await?;
        let reply: T = self.process.read_json_line().await?;
        self.supervisor.check_active().await?;
        if reply.sequence() != self.sequence {
            return Err(HarnessError::new("invalid_child_output"));
        }
        Ok(reply)
    }

    pub async fn stop(self) -> Result<()> {
        self.supervisor.stop(&self.process).await
    }
}

impl FixtureControl for Client {
    async fn stats(&mut self) -> Result<Stats> {
        Ok(self
            .command::<StatsReply>("stats", Value::Null)
            .await?
            .stats)
    }

    async fn configure(&mut self, settings: &Settings) -> Result<Stats> {
        let value =
            serde_json::to_value(settings).map_err(|_error| HarnessError::new("invalid_input"))?;
        Ok(self.command::<StatsReply>("configure", value).await?.stats)
    }

    async fn release(&mut self, all: bool) -> Result<Stats> {
        Ok(self
            .command::<StatsReply>("release", json!({"all":all}))
            .await?
            .stats)
    }

    async fn probe(&mut self, first: u32, second: u32) -> Result<Stats> {
        Ok(self
            .command::<StatsReply>("probe", json!({"first":first,"second":second}))
            .await?
            .stats)
    }

    async fn coverage(&mut self, ids: &[u32], calls: u32) -> Result<Coverage> {
        coverage(&BTreeMap::new(), ids, calls)?;
        let mut total = Coverage {
            auctions: 0,
            complete_auctions: 0,
        };
        for group in ids.chunks(512) {
            let row = self
                .command::<CoverageReply>("coverage", json!({"ids":group,"calls":calls}))
                .await?
                .coverage;
            if usize::try_from(row.auctions).ok() != Some(group.len())
                || row.complete_auctions > row.auctions
            {
                return Err(HarnessError::new("invalid_child_output"));
            }
            total.auctions = total
                .auctions
                .checked_add(row.auctions)
                .ok_or(HarnessError::new("fixture_failed"))?;
            total.complete_auctions = total
                .complete_auctions
                .checked_add(row.complete_auctions)
                .ok_or(HarnessError::new("fixture_failed"))?;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_readiness_requires_the_exact_protocol_a_bound_port_and_true_state() {
        let value: Ready =
            serde_json::from_value(json!({"ready":true,"protocol":1_i32,"port":12_345_i32}))
                .unwrap();
        assert_eq!(ready_port(value, 0).unwrap(), 12345);
        let matched =
            serde_json::from_value(json!({"ready":true,"protocol":1_i32,"port":12_345_i32}))
                .unwrap();
        assert_eq!(ready_port(matched, 12345).unwrap(), 12345);
        let mismatched =
            serde_json::from_value(json!({"ready":true,"protocol":1_i32,"port":12_345_i32}))
                .unwrap();
        assert_eq!(
            ready_port(mismatched, 12346).unwrap_err(),
            HarnessError::new("invalid_child_output")
        );
        let mut checked = 0_u8;
        for changed in [
            json!({"ready":false,"protocol":1_i32,"port":12_345_i32}),
            json!({"ready":true,"protocol":2_i32,"port":12_345_i32}),
            json!({"ready":true,"protocol":1_i32,"port":0_i32}),
        ] {
            assert_eq!(
                ready_port(serde_json::from_value(changed).unwrap(), 0).unwrap_err(),
                HarnessError::new("invalid_child_output")
            );
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 3);
        let unknown = serde_json::to_string(
            &json!({"ready":true,"protocol":1_u8,"port":12_345_u16,"url":"SECRET_CANARY"}),
        )
        .unwrap();
        for malformed in [
            r#"{"ready":true,"ready":false,"protocol":1,"port":12345}"#,
            unknown.as_str(),
        ] {
            assert!(serde_json::from_str::<Ready>(malformed).is_err());
        }
    }
}
