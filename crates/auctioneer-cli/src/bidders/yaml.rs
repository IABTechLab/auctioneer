use super::Error;
use serde_yaml_ng::Value;
use std::str::from_utf8;
use yaml_rust2::parser::{Event, Parser};

const MAX_BYTES: usize = 0x0001_0000;
const MAX_NODES: usize = 8_192;
const MAX_DEPTH: usize = 32;

pub(super) fn parse(bytes: &[u8]) -> Result<Value, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::SourceLimit);
    }
    let text = from_utf8(bytes).map_err(|_error| Error::InvalidSource)?;
    let mut parser = Parser::new_from_str(text);
    let mut nodes = 0_usize;
    let mut depth = 0_usize;
    let mut documents = 0_usize;
    loop {
        let (event, _marker) = parser.next_token().map_err(|_error| Error::InvalidSource)?;
        match event {
            Event::Scalar(_, _, anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(Error::InvalidSource);
                }
                nodes = nodes.checked_add(1).ok_or(Error::SourceLimit)?;
            }
            Event::MappingStart(anchor, tag) | Event::SequenceStart(anchor, tag) => {
                if anchor != 0 || tag.is_some() {
                    return Err(Error::InvalidSource);
                }
                nodes = nodes.checked_add(1).ok_or(Error::SourceLimit)?;
                depth = depth.checked_add(1).ok_or(Error::SourceLimit)?;
            }
            Event::MappingEnd | Event::SequenceEnd => {
                depth = depth.checked_sub(1).ok_or(Error::InvalidSource)?;
            }
            Event::DocumentStart => {
                documents = documents.checked_add(1).ok_or(Error::SourceLimit)?;
                if documents != 1 {
                    return Err(Error::InvalidSource);
                }
            }
            Event::Alias(_) | Event::Nothing => return Err(Error::InvalidSource),
            Event::StreamEnd => break,
            Event::StreamStart | Event::DocumentEnd => {}
        }
        if nodes > MAX_NODES || depth > MAX_DEPTH {
            return Err(Error::SourceLimit);
        }
    }
    if documents != 1 || depth != 0 {
        return Err(Error::InvalidSource);
    }
    let value: Value = serde_yaml_ng::from_slice(bytes).map_err(|_error| Error::InvalidSource)?;
    if !value.is_mapping() {
        return Err(Error::InvalidSource);
    }
    let mut pending = vec![&value];
    while let Some(node) = pending.pop() {
        match node {
            Value::Mapping(object) => {
                for (key, child) in object {
                    let name = key.as_str().ok_or(Error::InvalidSource)?;
                    if name == "<<" {
                        return Err(Error::InvalidSource);
                    }
                    pending.push(child);
                }
            }
            Value::Sequence(items) => pending.extend(items),
            Value::Tagged(_) => return Err(Error::InvalidSource),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_indicators_comments_and_block_text_remain_plain_data() {
        let parsed = parse(b"# &anchor !tag *alias\nendpoint: 'https://example.invalid/?a=1&b=2'\nextra_info: |\n  &not_an_anchor\n  !not_a_tag\n  *not_an_alias\n").unwrap();
        assert_eq!(
            parsed["endpoint"].as_str(),
            Some("https://example.invalid/?a=1&b=2")
        );
        assert!(
            parsed["extra_info"]
                .as_str()
                .unwrap()
                .contains("*not_an_alias")
        );
    }

    #[test]
    fn anchors_aliases_tags_and_merge_keys_are_never_expanded() {
        for bytes in [
            &b"x: &a {}\n"[..],
            b"x: &a {}\ny: *a\n",
            b"x: !custom value\n",
            b"x: !!str 1\n",
            b"x: {<<: {a: 1}}\n",
            b"'<<': {}\n",
            b"%TAG !e! tag:example.invalid,2026:\n---\nx: !e!foo value\n",
        ] {
            assert_eq!(parse(bytes), Err(Error::InvalidSource), "{bytes:?}");
        }
    }

    #[test]
    fn duplicate_keys_multiple_documents_and_nonstring_keys_are_refused() {
        for bytes in [
            &b"x: 1\nx: 2\n"[..],
            b"x: {a: 1, a: 2}\n",
            b"---\nx: 1\n---\nx: 2\n",
            b"[a, b]\n",
            b"1: scalar_key\n",
            b"? [a, b]\n: complex_key\n",
            b"x: [unterminated\n",
        ] {
            assert_eq!(parse(bytes), Err(Error::InvalidSource), "{bytes:?}");
        }
    }

    #[test]
    fn input_bytes_and_nodes_have_exact_inclusive_limits() {
        let mut bytes = format!("x: 1\n#{}", "x".repeat(0xfffa));
        assert_eq!(bytes.len(), 0x0001_0000);
        parse(bytes.as_bytes()).unwrap();
        bytes.push('x');
        assert_eq!(parse(bytes.as_bytes()), Err(Error::SourceLimit));
        // Root mapping, one key and one sequence, followed by scalar members.
        for (members, allowed) in [(8_188, true), (8_189, true), (8_190, false)] {
            let text = format!("x: [{}]\n", vec!["0"; members].join(","));
            let result = parse(text.as_bytes());
            if allowed {
                result.unwrap();
            } else {
                assert_eq!(result, Err(Error::SourceLimit));
            }
        }
    }

    #[test]
    fn depth_is_checked_before_tree_construction_at_both_boundaries() {
        for (sequences, allowed) in [(30, true), (31, true), (32, false)] {
            let text = format!("x: {}0{}\n", "[".repeat(sequences), "]".repeat(sequences));
            let result = parse(text.as_bytes());
            if allowed {
                result.unwrap();
            } else {
                assert_eq!(result, Err(Error::SourceLimit));
            }
        }
    }
}
