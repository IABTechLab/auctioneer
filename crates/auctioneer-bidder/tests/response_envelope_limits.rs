//! Response-owned limits cannot be replaced after decode admission is spent.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::{BidderError, BidderErrorKind, BidderHttpResponse};
    use serde_json::{Value, json};

    fn fake_body(document: &Value) -> Vec<u8> {
        serde_json::to_vec(document).expect("synthetic response JSON")
    }

    fn fake_response(nodes: u32) -> BidderHttpResponse {
        BidderHttpResponse::new(
            200,
            Vec::new(),
            fake_body(&json!({"id":"fake_response", "seatbid":[]})),
        )
        .within_node_budget(nodes)
    }

    fn assert_fake_error(error: &BidderError, code: u32, kind: BidderErrorKind, message: &str) {
        assert_eq!(error.code, code);
        assert_eq!(error.kind, kind);
        assert_eq!(error.message, message);
    }

    fn assert_fake_spent(response: &BidderHttpResponse) {
        let errors = response
            .parse_bid_response()
            .expect_err("limit replacement must not renew admission");
        assert_eq!(errors.len(), 1);
        assert_fake_error(
            &errors[0],
            4,
            BidderErrorKind::ResponseParse,
            "bidder response exceeded the decode budget",
        );
    }

    #[test]
    fn fake_limit_attachment_before_admission_preserves_the_first_exact_parse() {
        let response = fake_response(0)
            .within_node_budget(64)
            .within_node_budget(5);
        assert_eq!(response.max_parsed_nodes(), 5);
        let decoded = response.parse_bid_response().expect("five source nodes");
        assert_eq!(decoded.id, "fake_response");
        assert!(decoded.seatbid.is_empty());
    }

    #[test]
    fn fake_successful_admission_freezes_limits_while_the_first_tree_is_retained() {
        for replacement in [0, 4, 5, 6, u32::MAX] {
            let response = fake_response(5);
            let decoded = response.parse_bid_response().expect("first admitted tree");
            let reattached = response.within_node_budget(replacement);
            assert_eq!(reattached.max_parsed_nodes(), 5);
            assert_fake_spent(&reattached);
            assert_eq!(decoded.id, "fake_response");
        }
    }

    #[test]
    fn fake_dropped_output_does_not_reopen_limit_attachment() {
        let response = fake_response(5);
        drop(response.parse_bid_response().expect("first admitted tree"));
        let reattached = response.within_node_budget(u32::MAX).within_node_budget(0);
        assert_eq!(reattached.max_parsed_nodes(), 5);
        assert_fake_spent(&reattached);
    }

    #[test]
    fn fake_unfunded_admission_freezes_the_original_zero_limit() {
        let response = fake_response(0);
        let errors = response.parse_bid_response().expect_err("unfunded parse");
        assert_eq!(errors.len(), 1);
        assert_fake_error(
            &errors[0],
            4,
            BidderErrorKind::ResponseParse,
            "bidder response exceeded the JSON node budget",
        );
        let reattached = response.within_node_budget(u32::MAX);
        assert_eq!(reattached.max_parsed_nodes(), 0);
        assert_fake_spent(&reattached);
    }

    #[test]
    fn fake_structurally_refused_admission_freezes_the_original_limit() {
        let response = fake_response(4);
        let errors = response
            .parse_bid_response()
            .expect_err("one source node short");
        assert_eq!(errors.len(), 1);
        assert_fake_error(
            &errors[0],
            4,
            BidderErrorKind::ResponseParse,
            "bidder response exceeded the JSON node budget",
        );
        let reattached = response.within_node_budget(u32::MAX);
        assert_eq!(reattached.max_parsed_nodes(), 4);
        assert_fake_spent(&reattached);
    }

    #[test]
    fn fake_syntax_and_typed_refusals_freeze_the_original_limit() {
        let typed = fake_body(&json!({"id":"fake_response", "seatbid":false}));
        let mut syntax = fake_body(&json!({"id":"fake_response", "seatbid":[]}));
        assert_eq!(syntax.pop(), Some(b'}'));
        for body in [syntax, typed] {
            let response = BidderHttpResponse::new(200, Vec::new(), body).within_node_budget(64);
            let errors = response.parse_bid_response().expect_err("invalid source");
            assert_eq!(errors.len(), 1);
            assert_fake_error(
                &errors[0],
                14,
                BidderErrorKind::ResponseParse,
                "invalid bidder response",
            );
            let reattached = response.within_node_budget(u32::MAX);
            assert_eq!(reattached.max_parsed_nodes(), 64);
            assert_fake_spent(&reattached);
        }
    }

    #[test]
    fn fake_late_attachment_cannot_widen_creative_projection() {
        // Six nodes fit the byte cap but not the five-node decode allowance.
        let creative = json!([null, null, null, null, null]);
        assert_eq!(
            fake_response(6)
                .project_adm_value(&creative)
                .expect("independent exact-limit control"),
            creative.to_string(),
        );
        let response = fake_response(5);
        drop(response.parse_bid_response().expect("first admitted tree"));
        let reattached = response.within_node_budget(u32::MAX);
        let error = reattached
            .project_adm_value(&creative)
            .expect_err("creative helper must keep the original node limit");
        assert_fake_error(
            &error,
            4,
            BidderErrorKind::Other,
            "native creative exceeded parsing limits",
        );
    }

    #[test]
    fn fake_late_attachment_cannot_widen_native_unwrapping() {
        let source = json!({"native":{"assets":[null]}}).to_string();
        let mut control = source.clone();
        fake_response(6)
            .unwrap_native_adm(&mut control)
            .expect("independent exact-limit control");
        assert_eq!(control, json!({"assets":[null]}).to_string());

        let response = fake_response(5);
        drop(response.parse_bid_response().expect("first admitted tree"));
        let reattached = response.within_node_budget(u32::MAX);
        let mut adm = source.clone();
        let error = reattached
            .unwrap_native_adm(&mut adm)
            .expect_err("creative helper must keep the original node limit");
        assert_fake_error(
            &error,
            4,
            BidderErrorKind::Other,
            "native creative exceeded parsing limits",
        );
        assert_eq!(adm, source);
    }
}
