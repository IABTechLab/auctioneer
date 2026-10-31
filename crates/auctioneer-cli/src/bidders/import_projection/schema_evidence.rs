use serde::Deserialize;
use std::{ops::Range, path::Path, str};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    schema: String,
    #[serde(rename = "path")]
    _path: String,
    #[serde(rename = "blob")]
    _blob: String,
    #[serde(rename = "sha256")]
    _sha256: String,
    #[serde(rename = "bytes")]
    _bytes: usize,
    #[serde(rename = "valid_cases")]
    _valid_cases: usize,
    #[serde(rename = "invalid_cases")]
    _invalid_cases: usize,
    #[serde(rename = "valid_span")]
    _valid_span: Range<usize>,
    #[serde(rename = "invalid_span")]
    _invalid_span: Range<usize>,
    #[serde(rename = "verdict_counts")]
    _verdict_counts: [usize; 3],
    #[serde(rename = "previous")]
    _previous: bool,
    #[serde(rename = "original")]
    _original: bool,
    #[serde(rename = "duplicates")]
    _duplicates: Vec<(String, usize, usize)>,
    #[serde(rename = "lexical")]
    _lexical: Vec<(usize, String, String)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Helpers {
    schema: String,
    #[serde(rename = "helpers")]
    _helpers: Vec<Helper>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Helper {
    #[serde(rename = "index")]
    _index: usize,
    #[serde(rename = "prefixes")]
    _prefixes: usize,
    #[serde(rename = "onepass")]
    _onepass: bool,
    #[serde(rename = "alternative")]
    _alternative: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Workload {
    schema: String,
    #[serde(rename = "order")]
    _order: usize,
    #[serde(rename = "rows")]
    _rows: Vec<Case>,
    #[serde(default, rename = "maximum_string")]
    _maximum_string: bool,
    #[serde(default, rename = "maximum_collections")]
    _maximum_collections: bool,
    #[serde(default, rename = "string_refusals")]
    _string_refusals: bool,
    #[serde(default, rename = "pattern_initialization")]
    _pattern_initialization: bool,
    #[serde(default, rename = "retained_constructor")]
    _retained_constructor: bool,
    #[serde(default, rename = "fractional_allocation")]
    _fractional_allocation: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    #[serde(rename = "id")]
    _id: String,
    #[serde(rename = "input_json")]
    _input_json: String,
    #[serde(rename = "valid")]
    _valid: bool,
    #[serde(rename = "transforms")]
    _transforms: Vec<Transform>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Transform {
    MaxString {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "character")]
        _character: char,
        #[serde(rename = "invalid")]
        _invalid: bool,
    },
    MaxCollection {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "invalid")]
        _invalid: bool,
    },
    MaxProperties {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "invalid")]
        _invalid: bool,
    },
    Set {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "value")]
        _value: String,
    },
    Insert {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "key")]
        _key: String,
        #[serde(rename = "value")]
        _value: bool,
    },
    Pop {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "count")]
        _count: usize,
    },
    NestedArrays {
        #[serde(rename = "pointer")]
        _pointer: String,
        #[serde(rename = "depth")]
        _depth: usize,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Literals {
    schema: String,
    #[serde(rename = "cases")]
    _cases: Vec<Literal>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Literal {
    #[serde(rename = "input_json")]
    _input_json: String,
    #[serde(rename = "valid")]
    _valid: bool,
    #[serde(default, rename = "formats_disabled_valid")]
    _formats_disabled_valid: Option<bool>,
    #[serde(default, rename = "draft4_valid")]
    _draft4_valid: Option<bool>,
    #[serde(default, rename = "string_bytes")]
    _string_bytes: Option<StringBytes>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StringBytes {
    #[serde(rename = "pointer")]
    _pointer: String,
    #[serde(rename = "bytes")]
    _bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NumericType {
    schema: String,
    #[serde(rename = "input_json")]
    _input_json: String,
    #[serde(rename = "positive_json")]
    _positive_json: String,
    #[serde(rename = "valid")]
    _valid: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    schema: String,
    #[serde(rename = "valid_json")]
    _valid_json: String,
    #[serde(rename = "invalid_json")]
    _invalid_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclusion {
    name: String,
    #[serde(rename = "repository")]
    _repository: String,
    #[serde(rename = "revision")]
    _revision: String,
}

// Each DTO is deliberately consumed by Serde: every field is a checked part of
// the local data contract even when this projection only needs its owner.
fn declared_schema(filename: &str, text: &str) -> Option<String> {
    match filename {
        "parameters.pin.toml" => toml::from_str::<Pin>(text).ok().map(|value| value.schema),
        "helpers.toml" => toml::from_str::<Helpers>(text)
            .ok()
            .map(|value| value.schema),
        "workload.toml" => toml::from_str::<Workload>(text)
            .ok()
            .map(|value| value.schema),
        "literal-controls.toml" => toml::from_str::<Literals>(text)
            .ok()
            .map(|value| value.schema),
        "numeric-type.toml" => toml::from_str::<NumericType>(text)
            .ok()
            .map(|value| value.schema),
        "raw-controls.toml" => toml::from_str::<Raw>(text).ok().map(|value| value.schema),
        "inventory-exclusion.toml" => toml::from_str::<Exclusion>(text)
            .ok()
            .map(|value| value.name),
        _ => None,
    }
}

pub(super) fn owned(path: &Path, bytes: &[u8], schema_names: &[String]) -> bool {
    if bytes.len() > 64 * 1024 || path.parent() != Some(Path::new("tests/schema")) {
        return false;
    }
    let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Ok(text) = str::from_utf8(bytes) else {
        return false;
    };
    declared_schema(filename, text).is_some_and(|name| schema_names.contains(&name))
}
