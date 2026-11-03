//! Auctioneer's offline extraction of independently pinned `OpenRTB` standard membership.

use pulldown_cmark::{Event, OffsetIter, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::ops::Range;
use thiserror::Error;
use time::Date;
use time::macros::format_description;

const PINNED_COMMIT: &str = "1debba8001f40eb14597102dec3205afddf811d4";
const PINNED_RELEASE: &str = "2.6-202606";

#[derive(Clone, Copy)]
struct Scope {
    object: &'static str,
    section: &'static str,
}

const SCOPES: [Scope; 7] = [
    Scope {
        object: "Device",
        section: "3.2.18",
    },
    Scope {
        object: "Geo",
        section: "3.2.19",
    },
    Scope {
        object: "User",
        section: "3.2.20",
    },
    Scope {
        object: "EID",
        section: "3.2.27",
    },
    Scope {
        object: "UID",
        section: "3.2.28",
    },
    Scope {
        object: "UserAgent",
        section: "3.2.29",
    },
    Scope {
        object: "BrandVersion",
        section: "3.2.30",
    },
];

/// Human-authored provenance for the exact vendored Markdown bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceManifest {
    /// Immutable official source URL.
    pub url: String,
    /// Reviewed `OpenRTB` release.
    pub release: String,
    /// Full source revision.
    pub commit: String,
    /// Lowercase SHA-256 of the unmodified source bytes.
    pub sha256: String,
    /// Explicit human reviewer.
    pub reviewer: String,
    /// Explicit Gregorian review date, in YYYY-MM-DD format.
    pub review_date: String,
}

/// Membership only, in deterministic object/field order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// Standard field rows, with no privacy or review metadata.
    pub fields: Vec<FieldRow>,
}

/// One standard field, retaining the specification's object capitalization.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldRow {
    /// Specification object name, such as `UserAgent` or `EID`.
    pub object: String,
    /// Case-sensitive JSON member name.
    pub field: String,
    /// Canonical shape: string, integer, float, object, or their supported arrays.
    pub r#type: String,
    /// Exact scoped specification section number.
    pub section: String,
}

/// An invalid source, manifest, or scoped membership table.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct ImportError {
    message: String,
}

impl ImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Validate explicit provenance without manufacturing review metadata.
///
/// # Errors
/// Returns an error for missing, malformed, or unpinned provenance.
pub fn validate_manifest(manifest: &SourceManifest) -> Result<(), ImportError> {
    for (key, value) in [
        ("url", manifest.url.as_str()),
        ("release", manifest.release.as_str()),
        ("commit", manifest.commit.as_str()),
        ("sha256", manifest.sha256.as_str()),
        ("reviewer", manifest.reviewer.as_str()),
        ("review_date", manifest.review_date.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ImportError::new(format!("manifest {key} is empty")));
        }
    }
    if manifest.release != PINNED_RELEASE {
        return Err(ImportError::new(
            "manifest release does not match the pinned release",
        ));
    }
    if manifest.commit != PINNED_COMMIT {
        return Err(ImportError::new(
            "manifest commit does not match the pinned revision",
        ));
    }
    let blob_url = format!(
        "https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/{PINNED_COMMIT}/2.6.md"
    );
    let raw_url = format!(
        "https://raw.githubusercontent.com/InteractiveAdvertisingBureau/openrtb2.x/{PINNED_COMMIT}/2.6.md"
    );
    if manifest.url != blob_url && manifest.url != raw_url {
        return Err(ImportError::new(
            "manifest url must name the immutable official Markdown source",
        ));
    }
    if manifest.sha256.len() != 64
        || !manifest
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ImportError::new(
            "manifest sha256 must be 64 lowercase hexadecimal digits",
        ));
    }
    let date = Date::parse(
        &manifest.review_date,
        format_description!("[year]-[month]-[day]"),
    )
    .map_err(|_error| {
        ImportError::new("manifest review_date must be a Gregorian YYYY-MM-DD date")
    })?;
    if manifest.review_date.len() != 10 || date.year() < 1_i32 {
        return Err(ImportError::new(
            "manifest review_date must be a Gregorian YYYY-MM-DD date",
        ));
    }
    Ok(())
}

/// Verify the manifest and the digest of the exact source bytes.
///
/// # Errors
/// Returns an error for invalid provenance or a source digest mismatch.
pub fn validate_source_digest(manifest: &SourceManifest, source: &[u8]) -> Result<(), ImportError> {
    validate_manifest(manifest)?;
    if hex::encode(Sha256::digest(source)) != manifest.sha256 {
        return Err(ImportError::new(
            "source digest does not match manifest sha256",
        ));
    }
    Ok(())
}

/// Extract the seven scoped standard objects through structured GFM tables.
///
/// # Errors
/// Returns an error for missing, duplicate, empty, or malformed scoped tables.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "unrelated Markdown events have no membership meaning"
)]
pub fn extract_catalogue(source: &str) -> Result<Catalogue, ImportError> {
    let mut events = Parser::new_ext(source, Options::ENABLE_TABLES).into_offset_iter();
    let mut current = None;
    let mut sections = BTreeSet::new();
    let mut tables = BTreeSet::new();
    let mut fields = Vec::new();
    while let Some((event, _)) = events.next() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let heading = read_text(&mut events, TagEnd::Heading(level), TextContext::Heading)?;
                current = heading_scope(&heading)?;
                if let Some(scope) = current
                    && !sections.insert(scope.section)
                {
                    return Err(ImportError::new(format!(
                        "duplicate section {}",
                        scope.section
                    )));
                }
            }
            Event::Start(Tag::Table(columns)) => {
                if let Some(scope) = current {
                    if !tables.insert(scope.section) {
                        return Err(ImportError::new(format!(
                            "duplicate table in section {}",
                            scope.section
                        )));
                    }
                    if columns.len() != 3 {
                        return Err(ImportError::new(
                            "membership table must have exactly three columns",
                        ));
                    }
                    fields.extend(read_table(&mut events, source, scope)?);
                } else {
                    skip_container(&mut events, TagEnd::Table);
                }
            }
            Event::Start(tag) => skip_container(&mut events, tag.to_end()),
            _ => {}
        }
    }
    for scope in SCOPES {
        if !sections.contains(scope.section) {
            return Err(ImportError::new(format!(
                "missing section {} ({})",
                scope.section, scope.object
            )));
        }
        if !tables.contains(scope.section) {
            return Err(ImportError::new(format!(
                "missing table in section {}",
                scope.section
            )));
        }
    }
    fields.sort_unstable();
    Ok(Catalogue { fields })
}

fn heading_scope(heading: &str) -> Result<Option<Scope>, ImportError> {
    let mut words = heading.split_whitespace();
    let Some(section) = words.next() else {
        return Ok(None);
    };
    let Some(scope) = SCOPES.iter().find(|scope| scope.section == section) else {
        return Ok(None);
    };
    if words.next() != Some("-")
        || words.next() != Some("Object:")
        || words.next() != Some(scope.object)
        || words.next().is_some()
    {
        return Err(ImportError::new(format!(
            "section {section} must name Object: {}",
            scope.object
        )));
    }
    Ok(Some(*scope))
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "only container boundaries matter when skipping unrelated Markdown"
)]
fn skip_container(events: &mut OffsetIter<'_>, end: TagEnd) {
    let mut stack = vec![end];
    for (event, _) in events.by_ref() {
        match event {
            Event::Start(tag) => stack.push(tag.to_end()),
            Event::End(_) if stack.pop().is_none() || stack.is_empty() => return,
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TextContext {
    Heading,
    Membership,
    Description,
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "unsupported inline syntax fails closed in membership cells"
)]
fn read_text(
    events: &mut OffsetIter<'_>,
    end: TagEnd,
    context: TextContext,
) -> Result<String, ImportError> {
    let mut text = String::new();
    for (event, _) in events.by_ref() {
        match event {
            Event::End(tag) if tag == end => {
                return Ok(text.split_whitespace().collect::<Vec<_>>().join(" "));
            }
            Event::Text(value) | Event::Code(value) => text.push_str(&value),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::Html(value) | Event::InlineHtml(value) => {
                match value.trim().to_ascii_lowercase().as_str() {
                    "<br>" | "<br/>" | "<br />" => text.push(' '),
                    _ if context != TextContext::Membership => {}
                    _ => return Err(ImportError::new("unsupported HTML in membership cell")),
                }
            }
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Link { .. })
            | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link) => {}
            _ if context != TextContext::Membership => {}
            _ => return Err(ImportError::new("unsupported Markdown in membership cell")),
        }
    }
    Err(ImportError::new("unterminated Markdown heading or cell"))
}

fn read_row(
    events: &mut OffsetIter<'_>,
    source: &str,
    range: &Range<usize>,
    end: TagEnd,
) -> Result<Vec<String>, ImportError> {
    let mut cells = Vec::new();
    let mut last_cell_end = range.start;
    while let Some((event, cell_range)) = events.next() {
        if let Event::Start(Tag::TableCell) = event {
            if cell_range.is_empty() {
                return Err(ImportError::new("membership row contains a missing cell"));
            }
            let context = if cells.len() == 2 {
                TextContext::Description
            } else {
                TextContext::Membership
            };
            cells.push(read_text(events, TagEnd::TableCell, context)?);
            last_cell_end = cell_range.end;
        } else if event == Event::End(end) {
            let suffix = source
                .get(last_cell_end..range.end)
                .ok_or_else(|| ImportError::new("invalid Markdown row offsets"))?
                .trim();
            // GFM discards excess cells; offsets reveal unconsumed trailing columns.
            if cells.len() != 3 || !matches!(suffix, "" | "|") {
                return Err(ImportError::new(
                    "membership row must have exactly three cells",
                ));
            }
            return Ok(cells);
        } else {
            return Err(ImportError::new("unexpected Markdown in membership row"));
        }
    }
    Err(ImportError::new("unterminated membership row"))
}

fn read_table(
    events: &mut OffsetIter<'_>,
    source: &str,
    scope: Scope,
) -> Result<Vec<FieldRow>, ImportError> {
    let Some((Event::Start(Tag::TableHead), header_range)) = events.next() else {
        return Err(ImportError::new("missing membership table header"));
    };
    let header = read_row(events, source, &header_range, TagEnd::TableHead)?;
    if header != ["Attribute", "Type", "Description"] {
        return Err(ImportError::new(
            "membership header must be Attribute, Type, Description",
        ));
    }
    let mut fields = Vec::new();
    let mut names = BTreeSet::new();
    while let Some((event, row_range)) = events.next() {
        if event == Event::End(TagEnd::Table) {
            if fields.is_empty() {
                return Err(ImportError::new(format!(
                    "empty membership table in section {}",
                    scope.section
                )));
            }
            return Ok(fields);
        }
        if event != Event::Start(Tag::TableRow) {
            return Err(ImportError::new("unexpected Markdown in membership table"));
        }
        let cells = read_row(events, source, &row_range, TagEnd::TableRow)?;
        let mut cell_values = cells.into_iter();
        let field = cell_values
            .next()
            .ok_or_else(|| ImportError::new("missing field cell"))?;
        let kind = cell_values
            .next()
            .ok_or_else(|| ImportError::new("missing type cell"))?;
        validate_field(&field)?;
        if !names.insert(field.clone()) {
            return Err(ImportError::new(format!(
                "duplicate field {}.{field}",
                scope.object
            )));
        }
        fields.push(FieldRow {
            object: scope.object.to_owned(),
            field,
            r#type: canonical_type(&kind)?.to_owned(),
            section: scope.section.to_owned(),
        });
    }
    Err(ImportError::new("unterminated membership table"))
}

fn validate_field(field: &str) -> Result<(), ImportError> {
    let mut bytes = field.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        || !bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(ImportError::new(
            "field must be a nonempty lowercase JSON member name",
        ));
    }
    Ok(())
}

fn canonical_type(kind: &str) -> Result<&'static str, ImportError> {
    let base = if let Some((base, annotation)) = kind.split_once(';') {
        if !matches!(
            annotation.trim(),
            "recommended" | "required" | "DEPRECATED" | "default 0"
        ) {
            return Err(ImportError::new("unsupported type annotation"));
        }
        base.trim()
    } else {
        kind
    };
    match base {
        "string" => Ok("string"),
        "integer" | "int" => Ok("integer"),
        "float" => Ok("float"),
        "object" | "BrandVersion object" => Ok("object"),
        "object array" | "array of BrandVersion objects" => Ok("object array"),
        "string array" | "array of string" => Ok("string array"),
        _ => Err(ImportError::new("unsupported or empty field type")),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Catalogue, FieldRow, SourceManifest, extract_catalogue, validate_manifest,
        validate_source_digest,
    };
    use sha2::{Digest as _, Sha256};

    const PINNED_SOURCE: &str =
        include_str!("../../auctioneer-config-data/data/openrtb-2.6-source.md");
    const SCOPES: [(&str, &str); 7] = [
        ("Device", "3.2.18"),
        ("Geo", "3.2.19"),
        ("User", "3.2.20"),
        ("EID", "3.2.27"),
        ("UID", "3.2.28"),
        ("UserAgent", "3.2.29"),
        ("BrandVersion", "3.2.30"),
    ];

    fn manifest(source: &[u8]) -> SourceManifest {
        SourceManifest {
            url: "https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/1debba8001f40eb14597102dec3205afddf811d4/2.6.md".to_owned(),
            release: "2.6-202606".to_owned(),
            commit: "1debba8001f40eb14597102dec3205afddf811d4".to_owned(),
            sha256: hex::encode(Sha256::digest(source)),
            reviewer: "Explicit test reviewer".to_owned(),
            review_date: "2026-10-09".to_owned(),
        }
    }

    fn table(object: &str, section: &str, rows: &str) -> String {
        format!(
            "### {section} - Object: {object}\n\n| Attribute | Type | Description |\n| --- | --- | --- |\n{rows}\n\n"
        )
    }

    fn fixture(object: &str, rows: &str) -> String {
        SCOPES
            .iter()
            .map(|(name, section)| {
                table(
                    name,
                    section,
                    if *name == object {
                        rows
                    } else {
                        "| `ext` | object | Extension. |"
                    },
                )
            })
            .collect()
    }

    fn row<'a>(catalogue: &'a Catalogue, object: &str, field: &str) -> &'a FieldRow {
        catalogue
            .fields
            .iter()
            .find(|entry| entry.object == object && entry.field == field)
            .expect("expected independent field membership")
    }

    #[test]
    fn vendored_source_bytes_match_pinned_revision() {
        assert_eq!(
            hex::encode(Sha256::digest(PINNED_SOURCE.as_bytes())),
            "1b730796e2776d576cb5749706f6638fd4aa35594c1182e7d0e6aedbce605728",
            "vendored source must match the independently downloaded pinned revision"
        );
    }

    #[test]
    fn extracts_exact_pinned_membership_and_types() {
        let catalogue = extract_catalogue(PINNED_SOURCE).unwrap();
        let expected = [
            (
                "Device",
                "carrier connectiontype devicetype didmd5 didsha1 dnt dpidmd5 dpidsha1 ext flashver geo geofetch h hwv ifa ip ipv6 js langb language lmt macmd5 macsha1 make mccmnc model os osv ppi pxratio sua ua w",
            ),
            (
                "Geo",
                "accuracy city country ext ipservice lastfix lat lon metro region regionfips104 type utcoffset zip",
            ),
            (
                "User",
                "buyeruid consent customdata data eids ext gender geo id keywords kwarray yob",
            ),
            ("EID", "ext inserter matcher mm source uids"),
            ("UID", "atype ext id"),
            (
                "UserAgent",
                "architecture bitness browsers ext mobile model platform source",
            ),
            ("BrandVersion", "brand ext version"),
        ];
        assert_eq!(catalogue.fields.len(), 79);
        for (object, fields) in expected {
            let actual: Vec<&str> = catalogue
                .fields
                .iter()
                .filter(|entry| entry.object == object)
                .map(|entry| entry.field.as_str())
                .collect();
            assert_eq!(
                actual,
                fields.split_whitespace().collect::<Vec<_>>(),
                "{object}"
            );
        }
        assert_eq!(row(&catalogue, "EID", "mm").r#type, "integer");
        assert_eq!(
            row(&catalogue, "UserAgent", "browsers").r#type,
            "object array"
        );
        assert_eq!(row(&catalogue, "UserAgent", "platform").r#type, "object");
        assert_eq!(
            row(&catalogue, "BrandVersion", "version").r#type,
            "string array"
        );
        assert_eq!(row(&catalogue, "Device", "didsha1").r#type, "string");
        assert_eq!(row(&catalogue, "Device", "pxratio").r#type, "float");
        for (object, section) in SCOPES {
            assert!(
                catalogue
                    .fields
                    .iter()
                    .filter(|entry| entry.object == object)
                    .all(|entry| entry.section == section)
            );
        }
    }

    #[test]
    fn ignores_headings_and_tables_outside_scope() {
        let source = format!(
            "{}{}{}",
            table("Device", "7.1", "| id | mystery | Not scoped. |"),
            fixture("User", "| id | string | User ID. |"),
            table("Other", "3.2.31", "| id | mystery | Not scoped. |")
        );
        let catalogue = extract_catalogue(&source).unwrap();
        assert_eq!(catalogue.fields.len(), 7);
        assert!(
            catalogue
                .fields
                .iter()
                .all(|entry| entry.field != "mystery")
        );
    }

    #[test]
    fn extracts_inline_formatting_links_and_heading_anchors() {
        let source = fixture(
            "Device",
            "| [**`ua`**](https://example.test/ua) | *string* | Agent. |",
        );
        let formatted_source = source
            .replace(
                "Object: Device",
                "Object: **[Device](#objectdevice)** <a name=\"objectdevice\"></a>",
            )
            .replace("Attribute | Type", "**Attribute** | [Type](#types)");
        let catalogue = extract_catalogue(&formatted_source).unwrap();
        assert_eq!(row(&catalogue, "Device", "ua").r#type, "string");
    }

    #[test]
    fn extracts_eid_int_and_html_line_breaks() {
        let source = fixture(
            "EID",
            "| `mm` | int<br> | First line.<br />Second line. |\n| `uids` | object<br/>array | UID objects. |",
        );
        let catalogue = extract_catalogue(&source).unwrap();
        assert_eq!(row(&catalogue, "EID", "mm").r#type, "integer");
        assert_eq!(row(&catalogue, "EID", "uids").r#type, "object array");
    }

    #[test]
    fn rejects_duplicate_fields() {
        let source = fixture("User", "| id | string | One. |\n| **id** | string | Two. |");
        let error = extract_catalogue(&source).unwrap_err().to_string();
        assert!(error.contains("duplicate"), "{error}");
    }

    #[test]
    fn rejects_duplicate_sections_and_tables() {
        let duplicate_sections = format!(
            "{}{}",
            fixture("User", "| id | string | ID. |"),
            table("User", "3.2.20", "| id | string | ID. |")
        );
        assert!(
            extract_catalogue(&duplicate_sections)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        let duplicate_tables = fixture(
            "User",
            "| id | string | ID. |\n\n| Attribute | Type | Description |\n| --- | --- | --- |\n| ext | object | Extension. |",
        );
        assert!(
            extract_catalogue(&duplicate_tables)
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
    }

    #[test]
    fn rejects_missing_sections() {
        let source: String = SCOPES
            .iter()
            .filter(|(object, _)| *object != "Geo")
            .map(|(object, section)| table(object, section, "| ext | object | Extension. |"))
            .collect();
        assert!(
            extract_catalogue(&source)
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
    }

    #[test]
    fn rejects_empty_sections_and_absent_tables() {
        assert!(
            extract_catalogue(&fixture("Geo", ""))
                .unwrap_err()
                .to_string()
                .contains("empty")
        );
        let source = fixture("Geo", "").replace(
            "### 3.2.19 - Object: Geo\n\n| Attribute | Type | Description |\n| --- | --- | --- |\n",
            "### 3.2.19 - Object: Geo\n\nNo membership table.\n",
        );
        assert!(
            extract_catalogue(&source)
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
    }

    #[test]
    fn rejects_wrong_section_object_pair_and_case() {
        for replacement in ["3.2.19 - Object: Device", "3.2.18 - Object: device"] {
            let source = fixture("User", "| id | string | ID. |")
                .replace("3.2.18 - Object: Device", replacement);
            extract_catalogue(&source).expect_err(replacement);
        }
    }

    #[test]
    fn rejects_empty_or_invalid_field_names() {
        for field in ["", "two words", "user.id", "UserID"] {
            extract_catalogue(&fixture("User", &format!("| {field} | string | ID. |")))
                .expect_err(field);
        }
    }

    #[test]
    fn rejects_empty_unknown_and_partially_recognized_types() {
        for kind in [
            "",
            "boolean",
            "string or integer",
            "integer array",
            "object; surprise",
            "array of Alien objects",
            "int garbage",
        ] {
            extract_catalogue(&fixture("User", &format!("| id | {kind} | ID. |"))).expect_err(kind);
        }
    }

    #[test]
    fn rejects_malformed_headers_and_rows() {
        let source =
            fixture("User", "| id | string | ID. |").replace("Attribute | Type", "Name | Type");
        extract_catalogue(&source).unwrap_err();
        extract_catalogue(&fixture("User", "| id | string |")).unwrap_err();
        extract_catalogue(&fixture("User", "| id | string | ID. | Extra | ")).unwrap_err();
    }

    #[test]
    fn validates_manifest_and_exact_byte_digest() {
        let source = b"exact source\r\n";
        let manifest = manifest(source);
        validate_manifest(&manifest).unwrap();
        validate_source_digest(&manifest, source).unwrap();
        let error = validate_source_digest(&manifest, b"exact source\n")
            .unwrap_err()
            .to_string();
        assert!(error.contains("digest"), "{error}");
    }

    #[test]
    fn rejects_empty_review_metadata() {
        for key in [
            "url",
            "release",
            "commit",
            "sha256",
            "reviewer",
            "review_date",
        ] {
            let mut value = toml::Value::try_from(manifest(b"source")).unwrap();
            value[key] = toml::Value::String(" \t".to_owned());
            let manifest: SourceManifest = value.try_into().unwrap();
            let error = validate_manifest(&manifest).unwrap_err().to_string();
            assert!(error.contains(key), "{key}: {error}");
        }
    }

    #[test]
    fn rejects_malformed_or_unpinned_manifest_metadata() {
        let bad = [
            ("url", "https://example.test/2.6.md"),
            (
                "url",
                "https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md",
            ),
            ("release", "2.6-202605"),
            ("commit", "1debba8"),
            ("commit", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            ("sha256", "abcd"),
            (
                "sha256",
                "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            ),
            ("review_date", "2026-02-29"),
            ("review_date", "2026-13-01"),
            ("review_date", "2026-10-00"),
            ("review_date", "2026-1-09"),
            ("review_date", "0000-01-01"),
        ];
        for (key, text) in bad {
            let mut value = toml::Value::try_from(manifest(b"source")).unwrap();
            value[key] = toml::Value::String(text.to_owned());
            let manifest: SourceManifest = value.try_into().unwrap();
            let error = validate_manifest(&manifest).unwrap_err().to_string();
            assert!(error.contains(key), "{key}: {text}: {error}");
        }
        let mut manifest = manifest(b"source");
        manifest.review_date = "2024-02-29".to_owned();
        validate_manifest(&manifest).unwrap();
    }

    #[test]
    fn rejects_unknown_manifest_and_catalogue_metadata() {
        let encoded_manifest = toml::to_string(&manifest(b"source")).unwrap();
        toml::from_str::<SourceManifest>(&format!("{encoded_manifest}privacy = true\n"))
            .unwrap_err();
        let encoded_catalogue = "[[fields]]\nobject = 'User'\nfield = 'id'\ntype = 'string'\nsection = '3.2.20'\nclass = 'id'\n";
        toml::from_str::<Catalogue>(encoded_catalogue).unwrap_err();
    }

    #[test]
    fn generates_deterministic_sorted_membership_without_review_metadata() {
        let source = fixture("User", "| z | string | Last. |\n| a | integer | First. |");
        let mut reversed: Vec<String> = SCOPES
            .iter()
            .map(|(object, section)| {
                table(
                    object,
                    section,
                    if *object == "User" {
                        "| a | integer | First. |\n| z | string | Last. |"
                    } else {
                        "| ext | object | Extension. |"
                    },
                )
            })
            .collect();
        reversed.reverse();
        let first = extract_catalogue(&source).unwrap();
        let second = extract_catalogue(&reversed.concat()).unwrap();
        let bytes = toml::to_string(&first).unwrap();
        assert_eq!(bytes, toml::to_string(&second).unwrap());
        assert!(
            first
                .fields
                .windows(2)
                .all(|pair| matches!(pair, [left, right] if left < right))
        );
        assert_eq!(first, toml::from_str::<Catalogue>(&bytes).unwrap());
        for forbidden in [
            "reviewer",
            "review_date",
            "sha256",
            "privacy",
            "domains",
            "generated_at",
        ] {
            assert!(!bytes.contains(forbidden), "{forbidden}");
        }
    }
}
