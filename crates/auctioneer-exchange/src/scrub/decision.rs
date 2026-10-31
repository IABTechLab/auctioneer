//! Pure actions shared by model slots and classified flattened values.

use super::{ChangeKind, ScrubDecisions};
use auctioneer_config::disclosure::{Delegate, FieldRule, Rule, SchemaId, Transform};
use auctioneer_config::types::IpKeepBits;

/// Resolved location disclosure; COPPA dominates coarsening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoMode {
    Pass,
    Coarsen,
    RemoveAll,
}

/// Immutable composition of the caller's already-resolved eight decisions.
#[derive(Clone, Copy, Debug)]
pub struct Disclosure {
    pub deny_ids: bool,
    pub geo: GeoMode,
    pub deny_user_fpd: bool,
    pub deny_tid: bool,
    pub ip_keep_bits: IpKeepBits,
}

impl Disclosure {
    #[must_use]
    pub const fn resolve(decisions: ScrubDecisions, ip_keep_bits: IpKeepBits) -> Self {
        let ScrubDecisions {
            pass_id,
            pass_geo,
            transmit_ufpd,
            transmit_precise_geo,
            transmit_tid,
            ccpa,
            lmt,
            coppa,
        } = decisions;
        Self {
            deny_ids: !pass_id || !transmit_ufpd || ccpa || lmt || coppa,
            geo: if coppa {
                GeoMode::RemoveAll
            } else if !pass_geo || !transmit_precise_geo || ccpa || lmt {
                GeoMode::Coarsen
            } else {
                GeoMode::Pass
            },
            deny_user_fpd: !transmit_ufpd || coppa,
            deny_tid: !transmit_tid,
            ip_keep_bits,
        }
    }

    #[must_use]
    pub const fn protected(self) -> bool {
        self.deny_ids || !matches!(self.geo, GeoMode::Pass)
    }

    #[must_use]
    pub const fn unrestricted(self) -> bool {
        !self.protected() && !self.deny_user_fpd && !self.deny_tid
    }
}

/// Representation supplied by the accessor, not a second policy inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Input {
    Missing,
    Null,
    Valid,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Keep,
    Remove(ChangeKind),
    Visit(SchemaId),
    Transform(Transform),
}

/// There is deliberately no direct-domain fast path before context shape checks.
pub(crate) fn action(
    descriptor: Option<&FieldRule>,
    input: Input,
    disclosure: Disclosure,
) -> Action {
    if input == Input::Missing {
        return Action::Keep;
    }
    let Some(field) = descriptor else {
        return if disclosure.protected() {
            Action::Remove(ChangeKind::Unknown)
        } else {
            Action::Keep
        };
    };

    match field.rule {
        Rule::Delegate => {
            // Only the compiler's four explicitly owned branches can be opaque.
            return match field.delegate {
                Some(Delegate::UserFpd) if disclosure.deny_user_fpd => {
                    Action::Remove(ChangeKind::Fpd)
                }
                Some(Delegate::UserFpd) => {
                    if disclosure.protected() && input == Input::Invalid {
                        Action::Remove(ChangeKind::Malformed)
                    } else {
                        Action::Keep
                    }
                }
                // Policy compilation requires UserFpd for every Delegate row.
                // Keep this unreachable invariant failure closed, without a panic.
                None => Action::Remove(ChangeKind::Malformed),
            };
        }
        Rule::Id if disclosure.deny_ids => return Action::Remove(ChangeKind::Id),
        Rule::PreciseGeo if !matches!(disclosure.geo, GeoMode::Pass) => {
            return Action::Remove(ChangeKind::Geo);
        }
        Rule::Opaque if disclosure.protected() => return Action::Remove(ChangeKind::Unknown),
        Rule::Context
        | Rule::Control
        | Rule::Id
        | Rule::Child
        | Rule::Opaque
        | Rule::PreciseGeo
        | Rule::Transform => {}
    }
    if !disclosure.protected() {
        return Action::Keep;
    }
    if input == Input::Invalid {
        return Action::Remove(ChangeKind::Malformed);
    }
    if input == Input::Null {
        return if field.rule == Rule::Transform && !matches!(disclosure.geo, GeoMode::Pass) {
            Action::Remove(ChangeKind::Malformed)
        } else {
            Action::Keep
        };
    }
    match field.rule {
        Rule::Child => field
            .child
            .map_or(Action::Remove(ChangeKind::Malformed), Action::Visit),
        Rule::Context | Rule::Id => field.child.map_or(Action::Keep, Action::Visit),
        Rule::Transform if !matches!(disclosure.geo, GeoMode::Pass) => field
            .transform
            .map_or(Action::Remove(ChangeKind::Malformed), Action::Transform),
        Rule::Control | Rule::Opaque | Rule::PreciseGeo | Rule::Transform | Rule::Delegate => {
            Action::Keep
        }
    }
}

pub(crate) const fn clear_object(schema: SchemaId, disclosure: Disclosure) -> bool {
    matches!(schema, SchemaId::Geo) && matches!(disclosure.geo, GeoMode::RemoveAll)
}

#[cfg(test)]
mod tests {
    use super::{Action, Disclosure, GeoMode, Input, action};
    use crate::scrub::{ChangeKind, ScrubDecisions};
    use auctioneer_config::disclosure::{Delegate, FieldId, Rule, SchemaId, Shape, Transform};
    use auctioneer_config::types::IpKeepBits;

    const ALLOW: ScrubDecisions = ScrubDecisions {
        pass_id: true,
        pass_geo: true,
        transmit_ufpd: true,
        transmit_precise_geo: true,
        transmit_tid: true,
        ccpa: false,
        lmt: false,
        coppa: false,
    };
    const KEEP: IpKeepBits = IpKeepBits { v4: 24, v6: 56 };

    #[test]
    fn action_table_covers_every_rule_and_input_under_all_id_geo_combinations() {
        // Representatives of all dispositions, including context-only children.
        let fields = [
            FieldId::DeviceUa,
            FieldId::DeviceDnt,
            FieldId::UserId,
            FieldId::UserEids,
            FieldId::DeviceExt,
            FieldId::GeoExt,
            FieldId::GeoZip,
            FieldId::GeoLat,
            FieldId::DeviceIp,
            FieldId::DeviceIpv6,
            FieldId::UserKeywords,
            FieldId::UserKwarray,
            FieldId::UserData,
            FieldId::UserExtData,
            FieldId::DeviceExtPrebidInterstitial,
        ];
        for (ids, geo) in [(false, false), (false, true), (true, false), (true, true)] {
            let disclosure = Disclosure::resolve(
                ScrubDecisions {
                    pass_id: !ids,
                    pass_geo: !geo,
                    ..ALLOW
                },
                KEEP,
            );
            for id in fields {
                let field = id.descriptor();
                assert_eq!(
                    action(Some(field), Input::Missing, disclosure),
                    Action::Keep
                );
                for input in [Input::Null, Input::Valid, Input::Invalid] {
                    let expected = match field.rule {
                        Rule::Id if ids => Action::Remove(ChangeKind::Id),
                        Rule::PreciseGeo if geo => Action::Remove(ChangeKind::Geo),
                        Rule::Opaque if ids || geo => Action::Remove(ChangeKind::Unknown),
                        _ if !(ids || geo) => Action::Keep,
                        _ if input == Input::Invalid => Action::Remove(ChangeKind::Malformed),
                        Rule::Transform if geo && input == Input::Null => {
                            Action::Remove(ChangeKind::Malformed)
                        }
                        _ if input == Input::Null => Action::Keep,
                        Rule::Child => Action::Visit(field.child.unwrap()),
                        Rule::Id => field.child.map_or(Action::Keep, Action::Visit),
                        Rule::Transform if geo => Action::Transform(field.transform.unwrap()),
                        Rule::Context
                        | Rule::Control
                        | Rule::Opaque
                        | Rule::PreciseGeo
                        | Rule::Transform
                        | Rule::Delegate => Action::Keep,
                    };
                    assert_eq!(
                        action(Some(field), input, disclosure),
                        expected,
                        "{id:?} {input:?}"
                    );
                }
            }
            for input in [Input::Null, Input::Valid, Input::Invalid] {
                assert_eq!(
                    action(None, input, disclosure),
                    if ids || geo {
                        Action::Remove(ChangeKind::Unknown)
                    } else {
                        Action::Keep
                    }
                );
            }
        }
    }

    #[test]
    fn context_object_uses_declared_child_and_never_becomes_opaque() {
        let mut field = *FieldId::DeviceExt.descriptor();
        field.rule = Rule::Context;
        let disclosure = Disclosure::resolve(
            ScrubDecisions {
                pass_id: false,
                ..ALLOW
            },
            KEEP,
        );
        assert_eq!(
            action(Some(&field), Input::Valid, disclosure),
            Action::Visit(SchemaId::DeviceExt)
        );
        assert_eq!(
            action(Some(&field), Input::Invalid, disclosure),
            Action::Remove(ChangeKind::Malformed)
        );
    }

    #[test]
    fn resolved_removeall_fpd_tid_and_allow_controls_keep_separate_ownership() {
        let coppa = Disclosure::resolve(
            ScrubDecisions {
                coppa: true,
                ..ALLOW
            },
            KEEP,
        );
        assert!(coppa.deny_ids);
        assert_eq!(coppa.geo, GeoMode::RemoveAll);
        assert!(coppa.deny_user_fpd);
        assert!(!coppa.deny_tid);
        let fpd = Disclosure::resolve(
            ScrubDecisions {
                transmit_ufpd: false,
                ..ALLOW
            },
            KEEP,
        );
        assert!(fpd.deny_ids);
        assert_eq!(fpd.geo, GeoMode::Pass);
        for id in [
            FieldId::UserData,
            FieldId::UserKeywords,
            FieldId::UserKwarray,
            FieldId::UserExtData,
        ] {
            assert_eq!(id.descriptor().delegate, Some(Delegate::UserFpd));
            assert_eq!(
                action(Some(id.descriptor()), Input::Valid, fpd),
                Action::Remove(ChangeKind::Fpd)
            );
            assert_eq!(
                action(Some(id.descriptor()), Input::Valid, coppa),
                Action::Remove(ChangeKind::Fpd)
            );
        }
        let tid = Disclosure::resolve(
            ScrubDecisions {
                transmit_tid: false,
                ..ALLOW
            },
            KEEP,
        );
        assert!(!tid.protected());
        assert!(tid.deny_tid);
        assert_eq!(action(None, Input::Invalid, tid), Action::Keep);
        assert!(Disclosure::resolve(ALLOW, KEEP).unrestricted());
        assert_eq!(FieldId::UserKwarray.descriptor().shape, Shape::StringArray);
        assert_eq!(
            FieldId::GeoLat.descriptor().transform,
            Some(Transform::Coordinate)
        );
    }
}
