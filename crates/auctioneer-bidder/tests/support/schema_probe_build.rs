fn main() {
    if std::env::var("TARGET").as_deref() == Ok("wasm32-unknown-unknown") {
        println!("cargo:rustc-link-arg-bin=auctioneer-schema-link-probe=--export=auctioneer_schema_validate");
        println!("cargo:rustc-link-arg-bin=auctioneer-schema-link-probe=--export=auctioneer_schema_membership");
    }
}
