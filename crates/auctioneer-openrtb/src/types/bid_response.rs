//! `BidResponse` + `SeatBid` + `Bid`.

use serde::{Deserialize, Serialize};

use crate::ext::bid::ExtBid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidResponse {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seatbid: Vec<SeatBid>,
    /// `nbr` — present only when no live bidders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbr: Option<i32>,
    #[serde(default)]
    pub cur: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SeatBid {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub seat: String,
    #[serde(default, skip_serializing_if = "i32_zero")]
    pub group: i32,
    pub bid: Vec<Bid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde `skip_serializing_if` requires `fn(&T) -> bool` signature"
)]
fn i32_zero(value: &i32) -> bool {
    *value == 0_i32
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bid {
    pub id: String,
    pub impid: String,
    pub price: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "dealid")]
    pub deal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<ExtBid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn bidresponse_with_seatbid_round_trips() {
        let json = serde_json::json!({"id":"r1","seatbid":[{"seat":"fake_x","bid":[{"id":"b1","impid":"i1","price":1.23_f64}]}],"cur":"USD"});
        let resp: BidResponse = round_trip(&json);
        assert_eq!(resp.seatbid[0].seat, "fake_x");
        assert!((resp.seatbid[0].bid[0].price - 1.23).abs() < f64::EPSILON);
    }

    #[test]
    fn seat_identifier_is_optional_without_becoming_an_implicit_bidder_name() {
        let input = serde_json::json!({"id":"r", "cur":"USD", "seatbid":[
            {"bid":[{"id":"b", "impid":"i", "price":1.25_f64}]},
        ]});
        let response: BidResponse = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(response.seatbid.len(), 1);
        assert!(response.seatbid[0].seat.is_empty());
        assert_eq!(response.seatbid[0].bid[0].id, "b");
        assert_eq!(serde_json::to_value(response).unwrap(), input);
    }
}
