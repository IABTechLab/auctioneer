//! Borrowed, ordered activity evaluation using the shared configuration schema.

use std::collections::BTreeMap;

pub use auctioneer_config_data::activity::Activity;
use auctioneer_config_data::activity::{ActivityConfig, ComponentType, Condition, simple_fold_eq};

/// A target the activity rule applies to. Lifetime-borrowed so callers can
/// hand-tag bidder names from request-scoped slices without allocating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component<'a> {
    Bidder(&'a str),
    Analytics(&'a str),
    Rtd(&'a str),
    General(&'a str),
}

impl Component<'_> {
    fn matches(self, condition: &Condition) -> bool {
        let (name, kind) = match self {
            Self::Bidder(name) => (name, ComponentType::Bidder),
            Self::Analytics(name) => (name, ComponentType::Analytics),
            Self::Rtd(name) => (name, ComponentType::Rtd),
            Self::General(name) => (name, ComponentType::General),
        };
        (condition.component_name.is_empty()
            || condition
                .component_name
                .iter()
                .any(|candidate| simple_fold_eq(candidate, name)))
            && (condition.component_type.is_empty() || condition.component_type.contains(&kind))
    }
}

/// Evaluate the first matching rule, then the activity default (allow if absent).
/// Names are participating identities: an alias does not inherit its core name.
#[must_use]
pub fn activity_allowed(
    activities: &BTreeMap<Activity, ActivityConfig>,
    activity: Activity,
    component: Component<'_>,
) -> bool {
    let Some(config) = activities.get(&activity) else {
        return true;
    };
    for rule in config.rules.iter() {
        if rule
            .condition
            .as_ref()
            .is_none_or(|condition| component.matches(condition))
        {
            return rule.allow;
        }
    }
    config.default.unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::{Activity, Component};

    #[test]
    fn activity_and_component_equality() {
        assert_eq!(Activity::FetchBids, Activity::FetchBids);
        assert_eq!(
            Component::Bidder("fake_alias"),
            Component::Bidder("fake_alias")
        );
        assert_ne!(
            Component::Bidder("fake_alias"),
            Component::Bidder("fake_other")
        );
        assert_ne!(
            Component::General("fake_alias"),
            Component::Bidder("fake_alias")
        );
    }
}
