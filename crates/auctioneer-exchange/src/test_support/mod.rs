//! Test fixtures excluded from ordinary serving builds.

pub mod privacy_fixture;
mod response_budgets;

pub use response_budgets::response_budgets;

#[cfg(test)]
mod tests {
    use auctioneer_runtime::test_support::alloc_track;
    use std::hint::black_box;

    #[test]
    fn cumulative_counter_records_allocations_without_refunding_deallocations() {
        let before = alloc_track::allocated();
        let buffer = black_box(Vec::<u8>::with_capacity(4096));
        let allocated = alloc_track::allocated();
        assert!(allocated - before >= 4096);
        drop(buffer);
        assert_eq!(alloc_track::allocated(), allocated);
    }
}
