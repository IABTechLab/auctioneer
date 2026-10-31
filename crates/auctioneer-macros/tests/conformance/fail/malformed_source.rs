use auctioneer_macros::OpenRtbFieldMetadata;

macro_rules! malformed_model {
    () => {
        #[derive(OpenRtbFieldMetadata)]
        struct Root {
            pub payload: Option<
        }
    };
}

malformed_model!();

fn main() {}
