use super::super::{HarnessError, Result, io};
use crate::qualification::profile::Load;
use flate2::{Compression, GzBuilder};
use serde_json::Map;
use serde_json::{Value, json};
use std::io::Write as _;

#[derive(Clone, Copy, Default)]
pub struct AuctionOptions<'a> {
    pub id: u32,
    pub fpd: bool,
    pub debug: bool,
    pub consent: Option<&'a str>,
    pub dense_debug: bool,
}

pub fn aliases(calls: u32) -> Result<Vec<String>> {
    if !(1..=64).contains(&calls) {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok((0..calls).map(|i| format!("fake_{i:02}")).collect())
}

fn nodes(value: &Value) -> Result<u32> {
    let bytes = io::json_bytes(value, 1 << 20_u32)?;
    auctioneer_runtime::count_json_nodes(&bytes, 1 << 16_u32)
        .map_err(|_error| HarnessError::new("unreachable_shape"))
}

fn replace(value: &mut Value, path: &[&str], replacement: Value) -> Result<()> {
    let mut slot = value;
    for key in path {
        slot = slot
            .get_mut(*key)
            .ok_or(HarnessError::new("unreachable_shape"))?;
    }
    *slot = replacement;
    Ok(())
}

pub fn make_auction(load: Load, options: AuctionOptions<'_>) -> Result<Vec<u8>> {
    if !(1..=100).contains(&load.impressions)
        || !(1..=1 << 16_u32).contains(&load.nodes)
        || !(1..=1 << 20_u32).contains(&load.request_bytes)
        || options.id > 99_999_999
        || options
            .consent
            .is_some_and(|value| value.len() > 1 << 16_u32)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    let names = aliases(load.calls)?;
    let impressions = (0..load.impressions)
        .map(|index| {
            let bidders: Map<String, Value> =
                names.iter().map(|name| (name.clone(), json!({}))).collect();
            let mut ext = Map::from_iter([("bidder".to_owned(), Value::Object(bidders))]);
            if options.fpd {
                let fpd: Map<String, Value> = names
                    .iter()
                    .map(|name| (name.clone(), json!({"banner":{"ext":{"q2_fpd":true}}})))
                    .collect();
                ext.insert("prebid".to_owned(), json!({"imp":fpd}));
            }
            json!({"id":format!("i{index}"), "banner":{"w":300_u16,"h":250_u16}, "ext":ext})
        })
        .collect::<Vec<_>>();
    let mut value = json!({"id":format!("q{}", options.id.max(1)),"tmax":500_u16,
        "site":{"page":"https://publisher.example","publisher":{"id":"q2_allowed"}},
        "imp":impressions,"regs":{"gdpr":0_u8},"ext":{"prebid":{"debug":options.debug,"returnallbidstatus":true},"q2_nodes":[],"q2_pad":""}});
    if let Some(consent) = options.consent {
        let object = value
            .as_object_mut()
            .ok_or(HarnessError::new("unreachable_shape"))?;
        object.insert("regs".to_owned(), json!({"gdpr":1_u8}));
        object.insert("user".to_owned(), json!({"consent":consent}));
    }
    if options.dense_debug {
        let initial_banner = value
            .get_mut("imp")
            .and_then(Value::as_array_mut)
            .and_then(|imps| imps.first_mut())
            .and_then(|imp| imp.get_mut("banner"))
            .and_then(Value::as_object_mut)
            .ok_or(HarnessError::new("unreachable_shape"))?;
        initial_banner.insert("format".to_owned(), json!([]));
        let remaining = load
            .nodes
            .checked_sub(nodes(&value)?)
            .ok_or(HarnessError::new("unreachable_shape"))?;
        let count = remaining
            .checked_div(5)
            .ok_or(HarnessError::new("unreachable_shape"))?
            .min(8);
        let banner = value
            .get_mut("imp")
            .and_then(Value::as_array_mut)
            .and_then(|imps| imps.first_mut())
            .and_then(|imp| imp.get_mut("banner"))
            .and_then(Value::as_object_mut)
            .ok_or(HarnessError::new("unreachable_shape"))?;
        banner.insert(
            "format".to_owned(),
            Value::Array(vec![
                json!({"w":300_u16,"h":250_u16});
                usize::try_from(count).map_err(|_error| {
                    HarnessError::new("unreachable_shape")
                })?
            ]),
        );
    }
    let missing = load
        .nodes
        .checked_sub(nodes(&value)?)
        .ok_or(HarnessError::new("unreachable_shape"))?;
    replace(
        &mut value,
        &["ext", "q2_nodes"],
        json!(vec![
            0_u8;
            usize::try_from(missing).map_err(|_error| {
                HarnessError::new("unreachable_shape")
            })?
        ]),
    )?;
    let cap = usize::try_from(load.request_bytes)
        .map_err(|_error| HarnessError::new("unreachable_shape"))?;
    let unpadded = io::json_bytes(&value, 1 << 20_u32)?;
    let padding = cap
        .checked_sub(unpadded.len())
        .ok_or(HarnessError::new("unreachable_shape"))?;
    replace(
        &mut value,
        &["ext", "q2_pad"],
        Value::String("x".repeat(padding)),
    )?;
    io::json_bytes(&value, cap)
}

pub fn make_response(encoded_bytes: u64, decoded_bytes: u64, gzip: bool) -> Result<Vec<u8>> {
    if !(64..=1 << 20_u32).contains(&encoded_bytes) || !(128..=1 << 20_u32).contains(&decoded_bytes)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    let mut value = json!({"id":"q2","seatbid":[{"seat":"fake_bidder","bid":[{"id":"b","impid":"i0","price":1_u8,
        "w":300_u16,"h":250_u16,"mtype":1_u8,"adm":""}]}],"cur":"USD"});
    let cap =
        usize::try_from(decoded_bytes).map_err(|_error| HarnessError::new("unreachable_shape"))?;
    let padding = cap
        .checked_sub(io::json_bytes(&value, 1 << 20_u32)?.len())
        .ok_or(HarnessError::new("unreachable_shape"))?;
    let bid = value
        .get_mut("seatbid")
        .and_then(Value::as_array_mut)
        .and_then(|seats| seats.first_mut())
        .and_then(|seat| seat.get_mut("bid"))
        .and_then(Value::as_array_mut)
        .and_then(|bids| bids.first_mut())
        .and_then(Value::as_object_mut)
        .ok_or(HarnessError::new("unreachable_shape"))?;
    bid.insert("adm".to_owned(), Value::String("x".repeat(padding)));
    let raw = io::json_bytes(&value, cap)?;
    if !gzip {
        return Ok(raw);
    }
    let compress = |extra: Option<Vec<u8>>| -> Result<Vec<u8>> {
        let builder = GzBuilder::new().mtime(0);
        let header = if let Some(bytes) = extra {
            builder.extra(bytes)
        } else {
            builder
        };
        let mut encoder = header.write(Vec::new(), Compression::new(6));
        encoder
            .write_all(&raw)
            .map_err(|_error| HarnessError::new("fixture_failed"))?;
        encoder
            .finish()
            .map_err(|_error| HarnessError::new("fixture_failed"))
    };
    let base = compress(None)?;
    let size =
        usize::try_from(encoded_bytes).map_err(|_error| HarnessError::new("unreachable_shape"))?;
    let extra = size
        .checked_sub(base.len())
        .and_then(|bytes| bytes.checked_sub(2))
        .filter(|bytes| u16::try_from(*bytes).is_ok())
        .ok_or(HarnessError::new("unreachable_shape"))?;
    compress(Some(vec![0_u8; extra]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use std::collections::BTreeSet;
    use std::io::Read as _;

    fn maximum() -> Load {
        Load {
            concurrency: 2,
            calls: 16,
            request_bytes: 32 << 10_u32,
            nodes: 2048,
            impressions: 8,
            encoded_bytes: 16 << 10_u32,
            decoded_bytes: 32 << 10_u32,
        }
    }

    #[test]
    fn aliases_are_fake_prefixed_unique_and_bounded() {
        for calls in [1, 16, 64] {
            let names = aliases(calls).unwrap();
            assert_eq!(names.len(), usize::try_from(calls).unwrap());
            let unique: BTreeSet<_> = names.iter().collect();
            assert_eq!(unique.len(), names.len());
            assert!(
                names
                    .iter()
                    .all(|name| name.starts_with("fake_") && name.len() == 7)
            );
        }
        for calls in [0, 65] {
            assert_eq!(
                aliases(calls).unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
    }

    #[test]
    fn request_shapes_reach_exact_byte_node_impression_and_alias_targets() {
        for nodes in [512_u32, 1024, 2048] {
            for impressions in [1_u16, 8] {
                let load = Load {
                    nodes,
                    impressions,
                    ..maximum()
                };
                let bytes = make_auction(
                    load,
                    AuctionOptions {
                        id: 5,
                        ..AuctionOptions::default()
                    },
                )
                .unwrap();
                assert_eq!(bytes.len(), 32 << 10_u32);
                assert_eq!(
                    auctioneer_runtime::count_json_nodes(&bytes, 2048).unwrap(),
                    nodes
                );
                let parsed: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(parsed["id"], "q5");
                let imps = parsed["imp"].as_array().unwrap();
                assert_eq!(imps.len(), usize::from(impressions));
                for imp in imps {
                    assert_eq!(imp["ext"]["bidder"].as_object().unwrap().len(), 16);
                }
                assert_eq!(parsed["ext"]["prebid"]["returnallbidstatus"], true);
            }
        }
    }

    #[test]
    fn fpd_and_dense_debug_geometry_remain_separate_bounded_shapes() {
        let fpd = make_auction(
            Load {
                impressions: 1,
                ..maximum()
            },
            AuctionOptions {
                id: 1,
                fpd: true,
                ..AuctionOptions::default()
            },
        )
        .unwrap();
        let parsed: Value = serde_json::from_slice(&fpd).unwrap();
        assert_eq!(
            parsed["imp"][0]["ext"]["prebid"]["imp"]
                .as_object()
                .unwrap()
                .len(),
            16
        );
        let dense = make_auction(
            maximum(),
            AuctionOptions {
                id: 1,
                debug: true,
                dense_debug: true,
                ..AuctionOptions::default()
            },
        )
        .unwrap();
        let parsed_debug: Value = serde_json::from_slice(&dense).unwrap();
        assert_eq!(
            parsed_debug["imp"][0]["banner"]["format"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert!(parsed_debug["ext"]["q2_nodes"].as_array().unwrap().len() > 1000);
        assert_eq!(dense.len(), 32 << 10_u32);
        assert_eq!(
            auctioneer_runtime::count_json_nodes(&dense, 2048).unwrap(),
            2048
        );
    }

    #[test]
    fn legal_gzip_members_hit_encoded_and_decoded_caps_independently() {
        for (encoded, decoded) in [(4096_u64, 8192_u64), (16 << 10_u32, 32 << 10_u32)] {
            let member = make_response(encoded, decoded, true).unwrap();
            assert_eq!(u64::try_from(member.len()).unwrap(), encoded);
            let mut raw = Vec::new();
            GzDecoder::new(member.as_slice())
                .read_to_end(&mut raw)
                .unwrap();
            assert_eq!(u64::try_from(raw.len()).unwrap(), decoded);
            let parsed: Value = serde_json::from_slice(&raw).unwrap();
            assert_eq!(parsed["seatbid"][0]["seat"], "fake_bidder");
            assert_eq!(parsed["seatbid"][0]["bid"][0]["mtype"], 1_u8);
        }
    }

    #[test]
    fn impossible_or_over_cap_geometry_refuses_before_allocating() {
        for load in [
            Load {
                calls: 65,
                ..maximum()
            },
            Load {
                request_bytes: (1 << 20_u32) + 1,
                ..maximum()
            },
            Load {
                nodes: 1,
                ..maximum()
            },
            Load {
                impressions: 0,
                ..maximum()
            },
        ] {
            make_auction(load, AuctionOptions::default()).unwrap_err();
        }
        assert_eq!(
            make_response(64, 1 << 20_u32, true).unwrap_err(),
            HarnessError::new("unreachable_shape")
        );
    }
}
