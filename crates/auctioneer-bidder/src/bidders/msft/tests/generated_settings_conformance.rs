//! Generated-field conformance belongs with the declaring bidder.

#[cfg(test)]
mod tests {
    use crate::{BidderSettings, bidder_settings::msft};
    use serde_json::json;

    #[test]
    fn generated_settings_keep_owned_fields_and_implementation_key() {
        let settings: BidderSettings = serde_json::from_value(json!({
            "implementation": "msft", "settings": {}
        }))
        .unwrap();
        assert_eq!(settings.implementation_key(), "msft");
        let typed = settings.as_msft().unwrap();
        assert_eq!(typed.hb_source, None);
        assert_eq!(typed.hb_source_video, None);
        assert!(settings.as_thetradedesk().is_none());

        let wire = json!({
            "implementation": "msft", "settings": {"hb_source": 50_i32, "hb_source_video": 60_i32}
        });
        let populated: BidderSettings = serde_json::from_value(wire.clone()).unwrap();
        let view = msft::resolve(populated.as_msft(), None);
        assert_eq!(view.hb_source(), Some(50));
        assert_eq!(view.hb_source_video(), Some(60));
        assert_eq!(serde_json::to_value(&populated).unwrap(), wire);
    }

    #[test]
    fn settings_refuse_another_owners_field() {
        assert!(
            serde_json::from_value::<BidderSettings>(json!({
                "implementation": "msft", "settings": {"supply_id": "x"}
            }))
            .unwrap_err()
            .is_data()
        );
    }
}
