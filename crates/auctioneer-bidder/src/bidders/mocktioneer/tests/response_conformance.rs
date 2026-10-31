//! Sealed-decoder conformance for this implementation's registered factory.

use crate::response_budget::{NoContentPolicy, response_budget_tests};
use auctioneer_bidder::{Bidder, compiled_bidders};

fn bidder() -> Box<dyn Bidder> {
    let registration = compiled_bidders()
        .iter()
        .find(|row| row.key == "mocktioneer")
        .expect("the owning factory must be linked under its declared feature");
    (registration.factory)()
}

response_budget_tests!(
    bidder,
    "http://127.0.0.1:8787/openrtb2/auction",
    NoContentPolicy::ParseBody
);
