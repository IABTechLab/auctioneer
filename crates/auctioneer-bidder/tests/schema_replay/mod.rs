//! Fixed captured-input replay for isolated compiler probes, not a serving API.

pub const BATCH_ROWS: usize = 16;
pub const REFUSED: u64 = u64::MAX;

pub fn replay<I, C, F, V>(
    inputs: &[I],
    start: u32,
    reuse: bool,
    mut factory: F,
    mut validate: V,
) -> u64
where
    I: AsRef<str>,
    F: FnMut() -> Option<C>,
    V: FnMut(&serde_json::Value, &mut C) -> bool,
{
    let Ok(offset) = usize::try_from(start) else {
        return REFUSED;
    };
    if inputs.is_empty() || !offset.is_multiple_of(BATCH_ROWS) || offset >= inputs.len() {
        return REFUSED;
    }
    let Some(end) = offset
        .checked_add(BATCH_ROWS)
        .map(|end| end.min(inputs.len()))
    else {
        return REFUSED;
    };
    let begin = if reuse { 0 } else { offset };
    let mut owner = None;
    let mut packet = u64::from(u32::MAX);
    for (index, input) in inputs.iter().enumerate().take(end).skip(begin) {
        let code = match serde_json::from_str::<serde_json::Value>(input.as_ref()) {
            Err(_) => 2_u64,
            Ok(value) => {
                if !reuse || owner.is_none() {
                    owner = factory();
                }
                let Some(cache) = owner.as_mut() else {
                    return REFUSED;
                };
                u64::from(validate(&value, cache))
            }
        };
        if index >= offset {
            let Some(shift) = index
                .checked_sub(offset)
                .and_then(|ordinal| ordinal.checked_mul(2))
                .and_then(|bits| u32::try_from(bits).ok())
            else {
                return REFUSED;
            };
            packet = (packet & !(3_u64 << shift)) | (code << shift);
        }
    }
    packet
}

#[cfg(test)]
mod tests {
    use super::{REFUSED, replay};
    use serde_json::{Value, json};
    use std::cell::{Cell, RefCell};

    fn boolean(value: &Value, _: &mut ()) -> bool {
        value.as_bool().unwrap_or(false)
    }

    #[test]
    fn replay_preserves_schema_and_json_refusals_in_independent_literal_slots() {
        // Malformed input is intentional; no JSON value can represent it.
        let inputs = [
            json!(true).to_string(),
            json!(false).to_string(),
            "[".to_owned(),
        ];
        for reuse in [false, true] {
            assert_eq!(replay(&inputs, 0, reuse, || Some(()), boolean), 0xffff_ffe1);
        }
    }

    #[test]
    fn replay_full_batches_and_the_last_partial_batch_have_exact_padding() {
        for (input, expected) in [
            ("true", 0x5555_5555_u64),
            ("false", 0_u64),
            ("[", 0xaaaa_aaaa_u64),
        ] {
            let rows = vec![input; 17];
            for reuse in [false, true] {
                assert_eq!(replay(&rows, 0, reuse, || Some(()), boolean), expected);
                let last = match input {
                    "true" => 0xffff_fffd_u64,
                    "false" => 0xffff_fffc_u64,
                    _ => 0xffff_fffe_u64,
                };
                assert_eq!(replay(&rows, 16, reuse, || Some(()), boolean), last);
            }
        }
    }

    #[test]
    fn replay_refuses_empty_unaligned_and_out_of_range_batches() {
        assert_eq!(
            replay::<&str, (), _, _>(&[], 0, false, || Some(()), boolean),
            REFUSED
        );
        for rows in [vec!["true"], vec!["true"; 17]] {
            for start in [1_u32, 15, 17, 32, u32::MAX] {
                assert_eq!(replay(&rows, start, false, || Some(()), boolean), REFUSED);
            }
        }
    }

    #[test]
    fn replay_refuses_missing_owners_but_never_constructs_one_for_invalid_json() {
        assert_eq!(replay(&["true"], 0, false, || None::<()>, boolean), REFUSED);
        assert_eq!(replay(&["true"], 0, true, || None::<()>, boolean), REFUSED);
        let constructions = Cell::new(0_usize);
        for reuse in [false, true] {
            assert_eq!(
                replay(
                    &["["],
                    0,
                    reuse,
                    || {
                        constructions.set(constructions.get() + 1);
                        None::<()>
                    },
                    boolean
                ),
                0xffff_fffe
            );
        }
        assert_eq!(constructions.get(), 0);
    }

    #[test]
    fn replay_reuse_walks_the_actual_prefix_with_one_owner_and_skips_invalid_json() {
        let mut rows = vec!["true"; 19];
        rows[3] = "[";
        let constructions = Cell::new(0_usize);
        let states = RefCell::new(Vec::new());
        let packet = replay(
            &rows,
            16,
            true,
            || {
                constructions.set(constructions.get() + 1);
                Some(0_usize)
            },
            |_, state| {
                *state += 1;
                states.borrow_mut().push(*state);
                true
            },
        );
        assert_eq!(packet, 0xffff_ffd5);
        assert_eq!(constructions.get(), 1);
        assert_eq!(*states.borrow(), (1_usize..=18).collect::<Vec<_>>());
    }

    #[test]
    fn replay_fresh_walks_only_the_requested_batch_with_separate_owners() {
        let mut rows = vec!["true"; 19];
        rows[17] = "[";
        let constructions = Cell::new(0_usize);
        let states = RefCell::new(Vec::new());
        let packet = replay(
            &rows,
            16,
            false,
            || {
                constructions.set(constructions.get() + 1);
                Some(0_usize)
            },
            |_, state| {
                *state += 1;
                states.borrow_mut().push(*state);
                true
            },
        );
        assert_eq!(packet, 0xffff_ffd9);
        assert_eq!(constructions.get(), 2);
        assert_eq!(*states.borrow(), vec![1_usize, 1]);
    }
}
