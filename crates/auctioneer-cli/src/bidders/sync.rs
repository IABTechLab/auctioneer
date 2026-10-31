//! Reconcile source declarations, not runtime sync eligibility or endpoint approval.

use super::Error;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub(super) struct Declaration {
    key: Option<String>,
    supports: Option<Vec<String>>,
    iframe: Option<Value>,
    redirect: Option<Value>,
    #[serde(rename = "externalUrl")]
    external_url: Option<String>,
    #[serde(rename = "formatOverride")]
    format_override: Option<String>,
    #[serde(rename = "enabled")]
    _enabled: Option<bool>,
    #[serde(rename = "skipwhen")]
    skip_when: Option<Value>,
}

impl Declaration {
    // Pinned config/bidderinfo.go: Syncer.Defined excludes supports and enabled.
    fn defined(&self) -> bool {
        self.key.as_ref().is_some_and(|key| !key.is_empty())
            || self.iframe.is_some()
            || self.redirect.is_some()
            || self
                .external_url
                .as_ref()
                .is_some_and(|url| !url.is_empty())
            || self
                .format_override
                .as_ref()
                .is_some_and(|format| !format.is_empty())
            || self.skip_when.is_some()
    }

    fn family_key<'a>(&'a self, name: &'a str) -> &'a str {
        self.key
            .as_deref()
            .filter(|key| !key.is_empty())
            .unwrap_or(name)
    }

    fn endpoint_types(&self) -> Vec<&'static str> {
        let mut types = Vec::new();
        if self.iframe.is_some() {
            types.push("iframe");
        }
        if self.redirect.is_some() {
            types.push("redirect");
        }
        types
    }

    fn summary(&self, name: &str) -> Summary {
        let defined = self.defined();
        let origin = if defined {
            Origin::Declared
        } else if self
            .supports
            .as_ref()
            .is_some_and(|supports| !supports.is_empty())
        {
            Origin::SupportsOnly
        } else {
            Origin::DeclaredUndefined
        };
        Summary {
            origin,
            defined,
            family: defined.then(|| self.family_key(name).to_owned()),
            endpoint_types: self.endpoint_types(),
        }
    }
}

#[derive(Debug)]
pub(super) struct Member<'a> {
    pub name: &'a str,
    pub parent: Option<&'a str>,
    pub declared: Option<&'a Declaration>,
}

#[derive(Debug)]
pub(super) struct Resolved {
    pub by_name: BTreeMap<String, Summary>,
    pub families: Vec<Family>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Origin {
    Absent,
    Declared,
    InheritedKey,
    SupportsOnly,
    DeclaredUndefined,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(super) struct Summary {
    pub origin: Origin,
    pub defined: bool,
    pub family: Option<String>,
    pub endpoint_types: Vec<&'static str>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(super) struct Family {
    pub key: String,
    pub members: Vec<String>,
    pub endpoint_providers: Vec<String>,
}

pub(super) fn reconcile(members: &[Member<'_>]) -> Result<Resolved, Error> {
    let mut indexed = BTreeMap::new();
    for member in members {
        if indexed.insert(member.name, member).is_some() {
            return Err(Error::InvalidSource);
        }
    }

    let mut by_name = BTreeMap::new();
    for member in indexed.values() {
        let parent = member
            .parent
            .map(|name| indexed.get(name).copied().ok_or(Error::InvalidSource))
            .transpose()?;
        if parent.is_some_and(|base| base.parent.is_some()) {
            return Err(Error::InvalidSource);
        }
        let summary = if let Some(declared) = member.declared {
            declared.summary(member.name)
        } else if let Some(declared) = parent
            .and_then(|base| base.declared)
            .filter(|declared| declared.defined())
        {
            // Source aliases inherit a key only, never the parent's endpoint/settings block.
            let parent_name = member.parent.ok_or(Error::InvalidSource)?;
            Summary {
                origin: Origin::InheritedKey,
                defined: true,
                family: Some(declared.family_key(parent_name).to_owned()),
                endpoint_types: Vec::new(),
            }
        } else {
            Summary {
                origin: Origin::Absent,
                defined: false,
                family: None,
                endpoint_types: Vec::new(),
            }
        };
        by_name.insert(member.name.to_owned(), summary);
    }

    let mut families = BTreeMap::new();
    for (name, summary) in &by_name {
        if let Some(key) = &summary.family {
            let family = families.entry(key.clone()).or_insert_with(|| Family {
                key: key.clone(),
                members: Vec::new(),
                endpoint_providers: Vec::new(),
            });
            family.members.push(name.clone());
            if summary.origin == Origin::Declared && !summary.endpoint_types.is_empty() {
                family.endpoint_providers.push(name.clone());
            }
        }
    }
    Ok(Resolved {
        by_name,
        families: families.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Declaration, Family, Member, Origin, Summary, reconcile};
    use crate::bidders::Error;
    use serde::Deserialize;
    use serde_json::json;

    fn declaration(yaml: &str) -> Declaration {
        serde_yaml_ng::from_str(yaml).expect("valid declaration fixture")
    }

    fn member<'a>(
        name: &'a str,
        parent: Option<&'a str>,
        declared: Option<&'a Declaration>,
    ) -> Member<'a> {
        Member {
            name,
            parent,
            declared,
        }
    }

    fn summary(declared: Option<&Declaration>) -> Summary {
        reconcile(&[member("fake_base", None, declared)])
            .expect("valid membership")
            .by_name
            .remove("fake_base")
            .expect("base summary")
    }

    #[test]
    fn every_source_defined_predicate_is_independently_sufficient() {
        for (yaml, family, endpoints) in [
            ("key: shared", "shared", vec![]),
            ("iframe: {}", "fake_base", vec!["iframe"]),
            ("redirect: {}", "fake_base", vec!["redirect"]),
            ("externalUrl: https://sync.example", "fake_base", vec![]),
            ("formatOverride: b", "fake_base", vec![]),
            ("skipwhen: {}", "fake_base", vec![]),
        ] {
            let declared = declaration(yaml);
            assert_eq!(
                summary(Some(&declared)),
                Summary {
                    origin: Origin::Declared,
                    defined: true,
                    family: Some(family.to_owned()),
                    endpoint_types: endpoints,
                },
                "{yaml}",
            );
        }
    }

    #[test]
    fn empty_null_and_nondeterminative_fields_do_not_define_sync() {
        for (yaml, origin) in [
            ("{}", Origin::DeclaredUndefined),
            ("key: ''", Origin::DeclaredUndefined),
            ("iframe: null", Origin::DeclaredUndefined),
            ("redirect: null", Origin::DeclaredUndefined),
            ("externalUrl: ''", Origin::DeclaredUndefined),
            ("formatOverride: ''", Origin::DeclaredUndefined),
            ("skipwhen: null", Origin::DeclaredUndefined),
            ("supports: []", Origin::DeclaredUndefined),
            ("supports: [iframe]", Origin::SupportsOnly),
            ("enabled: true", Origin::DeclaredUndefined),
            ("enabled: false", Origin::DeclaredUndefined),
            ("enabled: null", Origin::DeclaredUndefined),
            (
                "supports: [iframe, redirect]\nenabled: true",
                Origin::SupportsOnly,
            ),
            ("unrecognized: {}", Origin::DeclaredUndefined),
        ] {
            let declared = declaration(yaml);
            assert_eq!(
                summary(Some(&declared)),
                Summary {
                    origin,
                    defined: false,
                    family: None,
                    endpoint_types: vec![],
                },
                "{yaml}",
            );
        }
        assert_eq!(summary(None).origin, Origin::Absent);
    }

    #[test]
    fn source_field_spellings_are_case_sensitive() {
        for yaml in [
            "Key: shared",
            "IFrame: {}",
            "Redirect: {}",
            "externalURL: https://sync.example",
            "external_url: https://sync.example",
            "format_override: b",
            "skipWhen: {}",
        ] {
            assert!(!summary(Some(&declaration(yaml))).defined, "{yaml}");
        }
    }

    #[test]
    fn supports_and_enabled_are_typed_but_do_not_disable_defined_declarations() {
        let declared = declaration("key: shared\nsupports: [iframe]\nenabled: false");
        assert!(summary(Some(&declared)).defined);
        for yaml in ["supports: {}", "enabled: not-a-boolean"] {
            serde_yaml_ng::from_str::<Declaration>(yaml).unwrap_err();
        }
    }

    #[test]
    fn absent_and_null_alias_declarations_inherit_only_the_defined_parent_key() {
        #[derive(Deserialize)]
        struct Metadata {
            #[serde(rename = "userSync")]
            declared: Option<Declaration>,
        }

        let base = declaration(
            "key: shared\niframe: {}\nredirect: {}\nexternalUrl: https://sync.example\nformatOverride: b\nskipwhen: {}\nenabled: false",
        );
        for yaml in ["{}", "userSync: null"] {
            let alias: Metadata = serde_yaml_ng::from_str(yaml).expect("alias metadata");
            let resolved = reconcile(&[
                member("fake_alias", Some("fake_base"), alias.declared.as_ref()),
                member("fake_base", None, Some(&base)),
            ])
            .expect("valid alias");
            assert_eq!(
                resolved.by_name["fake_alias"],
                Summary {
                    origin: Origin::InheritedKey,
                    defined: true,
                    family: Some("shared".to_owned()),
                    endpoint_types: vec![],
                },
            );
            assert_eq!(
                resolved.families,
                vec![Family {
                    key: "shared".to_owned(),
                    members: vec!["fake_alias".to_owned(), "fake_base".to_owned()],
                    endpoint_providers: vec!["fake_base".to_owned()],
                }],
            );
        }
    }

    #[test]
    fn inherited_family_defaults_to_parent_public_name() {
        for yaml in ["iframe: {}", "formatOverride: b", "skipwhen: {}"] {
            let base = declaration(yaml);
            let resolved = reconcile(&[
                member("fake_base", None, Some(&base)),
                member("fake_alias", Some("fake_base"), None),
            ])
            .expect("valid alias");
            assert_eq!(
                resolved.by_name["fake_alias"].family.as_deref(),
                Some("fake_base")
            );
            assert_eq!(
                resolved.by_name["fake_base"].family.as_deref(),
                Some("fake_base")
            );
        }
    }

    #[test]
    fn aliases_do_not_inherit_from_undefined_parents() {
        let empty = declaration("{}");
        let supports = declaration("supports: [iframe]");
        let enabled = declaration("enabled: true");
        for declared in [None, Some(&empty), Some(&supports), Some(&enabled)] {
            let resolved = reconcile(&[
                member("fake_base", None, declared),
                member("fake_alias", Some("fake_base"), None),
            ])
            .expect("valid alias");
            assert_eq!(resolved.by_name["fake_alias"].origin, Origin::Absent);
            assert!(!resolved.by_name["fake_alias"].defined);
            assert_eq!(resolved.by_name["fake_alias"].family, None);
            assert!(resolved.families.is_empty());
        }
    }

    #[test]
    fn explicit_undefined_alias_blocks_suppress_inheritance() {
        let base = declaration("key: shared\niframe: {}");
        for (yaml, origin) in [
            ("{}", Origin::DeclaredUndefined),
            ("supports: [iframe]", Origin::SupportsOnly),
            ("enabled: true", Origin::DeclaredUndefined),
            ("key: ''\niframe: null", Origin::DeclaredUndefined),
        ] {
            let alias = declaration(yaml);
            let resolved = reconcile(&[
                member("fake_base", None, Some(&base)),
                member("fake_alias", Some("fake_base"), Some(&alias)),
            ])
            .expect("valid explicit alias");
            assert_eq!(resolved.by_name["fake_alias"].origin, origin, "{yaml}");
            assert!(!resolved.by_name["fake_alias"].defined, "{yaml}");
            assert_eq!(resolved.by_name["fake_alias"].family, None, "{yaml}");
            assert_eq!(resolved.families[0].members, ["fake_base"]);
        }
    }

    #[test]
    fn own_alias_declaration_never_merges_parent_fields() {
        let base = declaration("key: shared\niframe: {}\nexternalUrl: https://sync.example");
        for (yaml, family, endpoints) in [
            ("redirect: {}", "fake_alias", vec!["redirect"]),
            ("formatOverride: i", "fake_alias", vec![]),
            ("key: shared", "shared", vec![]),
            ("key: own", "own", vec![]),
        ] {
            let alias = declaration(yaml);
            let resolved = reconcile(&[
                member("fake_base", None, Some(&base)),
                member("fake_alias", Some("fake_base"), Some(&alias)),
            ])
            .expect("valid own alias declaration");
            assert_eq!(
                resolved.by_name["fake_alias"],
                Summary {
                    origin: Origin::Declared,
                    defined: true,
                    family: Some(family.to_owned()),
                    endpoint_types: endpoints,
                },
                "{yaml}",
            );
        }
    }

    #[test]
    fn endpoint_providers_require_own_endpoint_fields_not_just_a_family_key() {
        let endpoints =
            declaration("key: shared\niframe: {}\nredirect: {}\nexternalUrl: https://sync.example");
        let key = declaration("key: shared");
        let external = declaration("key: shared\nexternalUrl: https://other.example");
        let resolved = reconcile(&[
            member("fake_key_only", None, Some(&key)),
            member("fake_inherited", Some("fake_provider"), None),
            member("fake_external", None, Some(&external)),
            member("fake_provider", None, Some(&endpoints)),
        ])
        .expect("shared family");
        assert_eq!(
            resolved.by_name["fake_provider"].endpoint_types,
            ["iframe", "redirect"],
        );
        assert_eq!(
            resolved.families,
            vec![Family {
                key: "shared".to_owned(),
                members: [
                    "fake_external",
                    "fake_inherited",
                    "fake_key_only",
                    "fake_provider"
                ]
                .map(str::to_owned)
                .to_vec(),
                endpoint_providers: vec!["fake_provider".to_owned()],
            }],
        );
    }

    #[test]
    fn family_keys_remain_case_sensitive_and_output_is_order_independent() {
        let upper = declaration("key: Shared");
        let lower = declaration("key: shared");
        let mut members = [
            member("fake_z_last", None, Some(&upper)),
            member("fake_a_first", None, Some(&upper)),
            member("fake_other", None, Some(&lower)),
            member("fake_absent", None, None),
        ];
        let first = reconcile(&members).expect("valid membership");
        members.reverse();
        let second = reconcile(&members).expect("valid reversed membership");
        assert_eq!(first.by_name, second.by_name);
        assert_eq!(first.families, second.families);
        assert_eq!(
            first
                .families
                .iter()
                .map(|family| family.key.as_str())
                .collect::<Vec<_>>(),
            ["Shared", "shared"],
        );
        assert_eq!(first.families[0].members, ["fake_a_first", "fake_z_last"]);
        assert_eq!(first.families[1].members, ["fake_other"]);
    }

    #[test]
    fn identical_endpoint_urls_do_not_infer_a_shared_family() {
        let declared = declaration("iframe: {url: 'https://sync.example'}");
        let resolved = reconcile(&[
            member("fake_first", None, Some(&declared)),
            member("fake_second", None, Some(&declared)),
        ])
        .expect("independent declarations");
        assert_eq!(
            resolved.by_name["fake_first"].family.as_deref(),
            Some("fake_first")
        );
        assert_eq!(
            resolved.by_name["fake_second"].family.as_deref(),
            Some("fake_second")
        );
        assert_eq!(resolved.families.len(), 2);
    }

    #[test]
    fn duplicate_identities_are_invalid() {
        let result = reconcile(&[
            member("fake_same", None, None),
            member("fake_same", None, None),
        ]);
        assert!(matches!(result, Err(Error::InvalidSource)));
    }

    #[test]
    fn missing_parents_are_invalid_even_with_an_own_declaration() {
        let declared = declaration("key: own");
        for own in [None, Some(&declared)] {
            assert!(matches!(
                reconcile(&[member("fake_alias", Some("fake_missing"), own)]),
                Err(Error::InvalidSource),
            ));
        }
    }

    #[test]
    fn alias_chains_and_cycles_are_invalid() {
        for members in [
            vec![
                member("fake_base", None, None),
                member("fake_alias", Some("fake_base"), None),
                member("fake_chain", Some("fake_alias"), None),
            ],
            vec![member("fake_self", Some("fake_self"), None)],
            vec![
                member("fake_first", Some("fake_second"), None),
                member("fake_second", Some("fake_first"), None),
            ],
        ] {
            assert!(matches!(reconcile(&members), Err(Error::InvalidSource)));
        }
    }

    #[test]
    fn summaries_and_families_have_stable_serialized_shapes() {
        for (origin, serialized) in [
            (Origin::Absent, "absent"),
            (Origin::Declared, "declared"),
            (Origin::InheritedKey, "inherited_key"),
            (Origin::SupportsOnly, "supports_only"),
            (Origin::DeclaredUndefined, "declared_undefined"),
        ] {
            assert_eq!(
                serde_json::to_value(origin).expect("origin JSON"),
                json!(serialized)
            );
        }
        assert_eq!(
            serde_json::to_value(summary(None)).expect("summary JSON"),
            json!({"origin": "absent", "defined": false, "family": null, "endpoint_types": []}),
        );
        assert_eq!(
            serde_json::to_value(Family {
                key: "shared".to_owned(),
                members: vec!["fake_base".to_owned()],
                endpoint_providers: vec![],
            })
            .expect("family JSON"),
            json!({"key": "shared", "members": ["fake_base"], "endpoint_providers": []}),
        );
    }

    #[test]
    fn empty_membership_resolves_without_families() {
        let resolved = reconcile(&[]).expect("empty membership");
        assert!(resolved.by_name.is_empty());
        assert!(resolved.families.is_empty());
    }
}
