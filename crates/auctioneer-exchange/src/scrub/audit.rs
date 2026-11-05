//! Fixed, identity-free disclosure diagnostics.

use auctioneer_config_data::disclosure::{SCHEMAS, SchemaId};

/// Aggregate reasons for actual changes, not attempted field visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Id,
    Geo,
    Coarsen,
    Fpd,
    Tid,
    Unknown,
    Malformed,
    Coppa,
}

impl ChangeKind {
    /// Fixed reason dimension; existing reason indexes remain stable.
    pub const COUNT: usize = 8;

    const fn index(self) -> usize {
        match self {
            Self::Id => 0,
            Self::Geo => 1,
            Self::Coarsen => 2,
            Self::Fpd => 3,
            Self::Tid => 4,
            Self::Unknown => 5,
            Self::Malformed => 6,
            Self::Coppa => 7,
        }
    }
}

/// Plain fixed counters with no payload, paths, or bidder identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[expect(
    clippy::partial_pub_fields,
    reason = "public scope status is required; private reason counters preserve saturated accounting"
)]
pub struct DisclosureAudit {
    counters: [u64; ChangeKind::COUNT],
    schema_counters: [[u64; ChangeKind::COUNT]; SCHEMAS.len()],
    pub attempted_scopes: u64,
    pub failed_scopes: u64,
    pub saturated: bool,
}

impl DisclosureAudit {
    #[must_use]
    pub const fn counts(&self) -> [u64; ChangeKind::COUNT] {
        self.counters
    }

    #[must_use]
    pub fn schema_counts(&self, schema: SchemaId) -> [u64; ChangeKind::COUNT] {
        self.schema_counters
            .get(schema.index())
            .copied()
            .unwrap_or_default()
    }

    pub fn record(&mut self, kind: ChangeKind) {
        if let Some(counter) = self.counters.get_mut(kind.index()) {
            add(counter, 1, &mut self.saturated);
        }
    }

    pub fn record_schema(&mut self, schema: SchemaId, kind: ChangeKind) {
        self.record(kind);
        if let Some(counter) = self
            .schema_counters
            .get_mut(schema.index())
            .and_then(|row| row.get_mut(kind.index()))
        {
            add(counter, 1, &mut self.saturated);
        }
    }

    pub fn start_scope(&mut self) {
        add(&mut self.attempted_scopes, 1, &mut self.saturated);
    }

    pub fn fail_scope(&mut self) {
        add(&mut self.failed_scopes, 1, &mut self.saturated);
    }

    pub fn merge(&mut self, other: &Self) {
        for (ours, theirs) in self.counters.iter_mut().zip(other.counters) {
            add(ours, theirs, &mut self.saturated);
        }
        for (our_row, incoming_row) in self.schema_counters.iter_mut().zip(&other.schema_counters) {
            for (ours, theirs) in our_row.iter_mut().zip(incoming_row) {
                add(ours, *theirs, &mut self.saturated);
            }
        }
        add(
            &mut self.attempted_scopes,
            other.attempted_scopes,
            &mut self.saturated,
        );
        add(
            &mut self.failed_scopes,
            other.failed_scopes,
            &mut self.saturated,
        );
        self.saturated |= other.saturated;
    }
}

fn add(counter: &mut u64, value: u64, saturated: &mut bool) {
    if let Some(next) = counter.checked_add(value) {
        *counter = next;
    } else {
        *counter = u64::MAX;
        *saturated = true;
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeKind, DisclosureAudit};
    use auctioneer_config_data::disclosure::SchemaId;
    use std::mem::needs_drop;

    #[test]
    fn schema_counts_merge_into_totals_without_identity_or_heap_state() {
        let mut earlier = DisclosureAudit::default();
        earlier.start_scope();
        earlier.record_schema(SchemaId::User, ChangeKind::Id);
        let mut failed = DisclosureAudit::default();
        failed.start_scope();
        failed.record_schema(SchemaId::Geo, ChangeKind::Malformed);
        failed.record_schema(SchemaId::Geo, ChangeKind::Coppa);
        failed.record(ChangeKind::Tid);
        failed.fail_scope();
        earlier.merge(&failed);
        assert_eq!(earlier.counts(), [1, 0, 0, 0, 1, 0, 1, 1]);
        assert_eq!(
            earlier.schema_counts(SchemaId::User),
            [1, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            earlier.schema_counts(SchemaId::Geo),
            [0, 0, 0, 0, 0, 0, 1, 1]
        );
        assert_eq!(earlier.attempted_scopes, 2);
        assert_eq!(earlier.failed_scopes, 1);
        assert!(!earlier.saturated);
        assert!(!needs_drop::<DisclosureAudit>());
    }

    #[test]
    fn only_diagnostic_counters_saturate_and_merges_keep_the_flag() {
        let mut audit = DisclosureAudit {
            attempted_scopes: u64::MAX,
            failed_scopes: u64::MAX,
            ..DisclosureAudit::default()
        };
        audit.counters[0] = u64::MAX;
        audit.schema_counters[SchemaId::User.index()][0] = u64::MAX;
        audit.start_scope();
        audit.fail_scope();
        audit.record_schema(SchemaId::User, ChangeKind::Id);
        assert_eq!(audit.counts()[0], u64::MAX);
        assert_eq!(audit.schema_counts(SchemaId::User)[0], u64::MAX);
        assert!(audit.saturated);
        let mut merged = DisclosureAudit::default();
        merged.merge(&audit);
        assert_eq!(merged, audit);
    }
}
