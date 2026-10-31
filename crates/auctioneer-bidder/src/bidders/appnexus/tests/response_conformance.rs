//! Sealed-decoder conformance for this implementation's registered factory.

use crate::response_budget::{NoContentPolicy, response_budget_tests};
use auctioneer_bidder::{Bidder, compiled_bidders};

fn bidder() -> Box<dyn Bidder> {
    let registration = compiled_bidders()
        .iter()
        .find(|row| row.key == "appnexus")
        .expect("the owning factory must be linked under its declared feature");
    (registration.factory)()
}

response_budget_tests!(
    bidder,
    "https://ib.adnxs.com/openrtb2",
    NoContentPolicy::ParseBody
);
