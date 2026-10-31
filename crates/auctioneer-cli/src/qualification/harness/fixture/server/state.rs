use super::super::super::{HarnessError, Result, native::geometry::make_response};
use super::super::{
    Control, Mode, Options, ProviderMode, ProviderStats, Settings, Stats, coverage, selected,
};
#[cfg(test)]
use super::tests::Hold;
use bytes::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
#[cfg(test)]
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Native,
    Provider(ProviderMode),
}

pub(super) struct State {
    kind: Kind,
    body_cap: usize,
    max_sockets: u16,
    running: bool,
    stats: Stats,
    provider: ProviderStats,
    max_requests: u32,
    offered: u32,
    active: BTreeMap<u32, u32>,
    seen: BTreeMap<u32, u32>,
    settings: Settings,
    response: Bytes,
    hold: bool,
    parked: VecDeque<Parked>,
    #[cfg(test)]
    test_hold: Option<Arc<Hold>>,
}

struct Parked {
    ticket: u32,
    release: oneshot::Sender<()>,
}

pub(super) struct Plan {
    pub mode: Mode,
    pub body: Bytes,
    pub delay_ms: u64,
    pub released: Option<oneshot::Receiver<()>>,
}

pub(super) struct Completion {
    id: u32,
    ticket: u32,
    mode: Mode,
    encoded_bytes: u64,
    decoded_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    all: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    first: u32,
    second: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Population {
    ids: Vec<u32>,
    calls: u32,
}

impl State {
    pub const fn limits(&self) -> (Kind, usize, u16) {
        (self.kind, self.body_cap, self.max_sockets)
    }

    pub const fn stop(&mut self) {
        self.running = false;
    }

    pub const fn running(&self) -> bool {
        self.running
    }

    #[cfg(test)]
    pub const fn stats(&self) -> &Stats {
        &self.stats
    }

    #[cfg(test)]
    pub fn test_hold(&self) -> Option<Arc<Hold>> {
        self.test_hold.clone()
    }

    #[cfg(test)]
    pub fn set_test_hold(&mut self, hold: Arc<Hold>) {
        self.test_hold = Some(hold);
    }

    pub fn new(options: &Options) -> Result<Self> {
        let (kind, body_cap, max_requests, max_sockets) = match options {
            Options::Native {
                body_cap,
                max_requests,
                max_sockets,
                ..
            } => {
                if !(128..=1 << 20_u32).contains(body_cap)
                    || !(1..=1_000_000).contains(max_requests)
                    || !(1..=128).contains(max_sockets)
                {
                    return Err(HarnessError::new("invalid_input"));
                }
                (Kind::Native, *body_cap, *max_requests, *max_sockets)
            }
            Options::Provider {
                scenario,
                max_requests,
                key_file,
                cert_file,
                ..
            } => {
                if !(1..=32).contains(max_requests) || key_file.is_some() != cert_file.is_some() {
                    return Err(HarnessError::new("invalid_input"));
                }
                (
                    Kind::Provider(*scenario),
                    16 << 10_u32,
                    u32::from(*max_requests),
                    8,
                )
            }
        };
        Ok(Self {
            kind,
            body_cap,
            max_requests,
            max_sockets,
            running: true,
            stats: Stats::default(),
            provider: ProviderStats::default(),
            offered: 0,
            active: BTreeMap::new(),
            seen: BTreeMap::new(),
            settings: Settings {
                mode: Mode::Plain,
                encoded_bytes: 1024,
                decoded_bytes: 1024,
                seed: 0,
                delay_ms: 0,
                stall_after_calls: 0,
            },
            response: Bytes::from(make_response(1024, 1024, false)?),
            hold: false,
            parked: VecDeque::new(),
            #[cfg(test)]
            test_hold: None,
        })
    }

    pub fn offer(&mut self) -> bool {
        // Saturate above the cap: even unlimited refused traffic cannot wrap it.
        self.offered = self
            .offered
            .saturating_add(1)
            .min(self.max_requests.saturating_add(1));
        self.running && self.offered <= self.max_requests
    }

    pub fn reject(&mut self) {
        if matches!(self.kind, Kind::Provider(_)) {
            self.provider.rejected = self.provider.rejected.saturating_add(1).min(1_000_000);
        }
    }

    pub fn dispatch(&mut self, case_id: u8) {
        self.provider.dispatches = self.provider.dispatches.saturating_add(1);
        self.provider.case_id = case_id;
    }

    pub fn begin(&mut self, id: u32, expanded: bool) -> Result<(Completion, Plan)> {
        if (!self.seen.contains_key(&id) && self.seen.len() >= 10_000)
            || self.stats.current_active_calls >= u32::from(self.max_sockets)
        {
            return Err(HarnessError::new("fixture_failed"));
        }
        let calls = self
            .seen
            .get(&id)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(HarnessError::new("fixture_failed"))?;
        let active_calls = self
            .active
            .get(&id)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(HarnessError::new("fixture_failed"))?;
        let ticket = self
            .stats
            .calls
            .checked_add(1)
            .ok_or(HarnessError::new("fixture_failed"))?;
        let mode = selected(
            self.settings.mode,
            id,
            calls,
            self.settings.stall_after_calls,
        );
        let (released, parked) = if self.hold {
            if self.parked.len() >= usize::from(self.max_sockets) {
                return Err(HarnessError::new("fixture_failed"));
            }
            let (release, receiver) = oneshot::channel();
            (Some(receiver), Some(Parked { ticket, release }))
        } else {
            (None, None)
        };
        let delay_ms = if self.hold {
            0
        } else if matches!(self.settings.mode, Mode::Cancel) && id.is_multiple_of(3) {
            10
        } else {
            self.settings.delay_ms
        };
        let body = if matches!(mode, Mode::Error) {
            Bytes::from_static(b"{}")
        } else {
            self.response.clone()
        };
        let encoded_bytes =
            u64::try_from(body.len()).map_err(|_error| HarnessError::new("fixture_failed"))?;
        self.seen.insert(id, calls);
        self.active.insert(id, active_calls);
        self.stats.calls = ticket;
        self.stats.current_active_calls = self.stats.current_active_calls.saturating_add(1);
        self.stats.max_calls_per_auction = self.stats.max_calls_per_auction.max(calls);
        self.stats.max_active_calls = self
            .stats
            .max_active_calls
            .max(self.stats.current_active_calls);
        let auctions = u32::try_from(self.active.len()).unwrap_or(u32::MAX);
        self.stats.max_active_auctions = self.stats.max_active_auctions.max(auctions);
        let debug = u32::try_from(
            self.active
                .keys()
                .filter(|key| (700_000..800_000).contains(*key))
                .count(),
        )
        .unwrap_or(u32::MAX);
        self.stats.max_active_debug_auctions = self.stats.max_active_debug_auctions.max(debug);
        if expanded {
            self.stats.fpd_expansions = self.stats.fpd_expansions.saturating_add(1);
        }
        if matches!(mode, Mode::Stall) {
            self.stats.stalled_reads = self.stats.stalled_reads.saturating_add(1);
        }
        if let Some(entry) = parked {
            self.parked.push_back(entry);
        }
        Ok((
            Completion {
                id,
                ticket,
                mode,
                encoded_bytes,
                decoded_bytes: self.settings.decoded_bytes,
            },
            Plan {
                mode,
                body,
                delay_ms,
                released,
            },
        ))
    }

    pub fn finish(&mut self, completion: &Completion, flushed: bool) {
        self.parked
            .retain(|entry| entry.ticket != completion.ticket);
        if let Some(count) = self.active.get_mut(&completion.id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.active.remove(&completion.id);
            }
        }
        self.stats.current_active_calls = self.stats.current_active_calls.saturating_sub(1);
        if !flushed {
            self.stats.disconnected = self.stats.disconnected.saturating_add(1);
            return;
        }
        self.stats.completed = self.stats.completed.saturating_add(1);
        // Preserve the legacy snapshot population, but only at an actual flush.
        self.stats.completed_auctions = u32::try_from(self.seen.len()).unwrap_or(u32::MAX);
        if !matches!(completion.mode, Mode::Error) {
            self.stats.max_encoded_bytes =
                self.stats.max_encoded_bytes.max(completion.encoded_bytes);
            self.stats.max_decoded_bytes =
                self.stats.max_decoded_bytes.max(completion.decoded_bytes);
            if matches!(completion.mode, Mode::Gzip) {
                self.stats.gzip_responses = self.stats.gzip_responses.saturating_add(1);
            }
        }
    }

    pub fn command(&mut self, control: Control) -> Result<Value> {
        if !(1..=1_000_000).contains(&control.id) {
            return Err(HarnessError::new("invalid_input"));
        }
        if !self.running {
            return Err(HarnessError::new("fixture_failed"));
        }
        if matches!(self.kind, Kind::Provider(_)) {
            if control.command != "snapshot" || !control.settings.is_null() {
                return Err(HarnessError::new("invalid_input"));
            }
            return Ok(
                json!({"id":control.id,"dispatches":self.provider.dispatches,
                "case_id":self.provider.case_id,"rejected":self.provider.rejected}),
            );
        }
        match control.command.as_str() {
            "configure" => self.configure(control.settings)?,
            "release" => {
                let release: Release = serde_json::from_value(control.settings)
                    .map_err(|_error| HarnessError::new("invalid_input"))?;
                self.release(release.all);
            }
            "probe" => {
                let probe: Probe = serde_json::from_value(control.settings)
                    .map_err(|_error| HarnessError::new("invalid_input"))?;
                if !(1..=999_999).contains(&probe.first) || !(1..=999_999).contains(&probe.second) {
                    return Err(HarnessError::new("invalid_input"));
                }
                self.stats.probe_first_calls = self.seen.get(&probe.first).copied().unwrap_or(0);
                self.stats.probe_second_calls = self.seen.get(&probe.second).copied().unwrap_or(0);
            }
            "coverage" => {
                let population: Population = serde_json::from_value(control.settings)
                    .map_err(|_error| HarnessError::new("invalid_input"))?;
                return Ok(
                    json!({"id":control.id,"coverage":coverage(&self.seen, &population.ids, population.calls)?}),
                );
            }
            "stats" if control.settings.is_null() => {}
            _ => return Err(HarnessError::new("invalid_input")),
        }
        Ok(json!({"id":control.id,"stats":self.stats}))
    }

    fn configure(&mut self, value: Value) -> Result<()> {
        let settings: Settings =
            serde_json::from_value(value).map_err(|_error| HarnessError::new("invalid_input"))?;
        if settings.delay_ms > 2000 || settings.stall_after_calls > 10_000 {
            return Err(HarnessError::new("invalid_input"));
        }
        if self.stats.current_active_calls != 0 {
            return Err(HarnessError::new("fixture_busy"));
        }
        let gzip = !matches!(settings.mode, Mode::Plain | Mode::Error);
        let response = make_response(settings.encoded_bytes, settings.decoded_bytes, gzip)?;
        self.hold = matches!(settings.mode, Mode::Park);
        self.settings = settings;
        self.response = Bytes::from(response);
        self.seen.clear();
        self.active.clear();
        self.parked.clear();
        self.stats = Stats::default();
        Ok(())
    }

    fn release(&mut self, all: bool) {
        if all {
            self.hold = false;
        }
        let limit = if all { self.parked.len() } else { 2 };
        let mut sent = 0_usize;
        while sent < limit {
            let Some(entry) = self.parked.pop_front() else {
                break;
            };
            if entry.release.send(()).is_ok() {
                sent = sent.saturating_add(1);
            }
        }
    }
}
