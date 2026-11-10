//! Deployment documentation gate for the `/_auctioneer/*` operational
//! endpoints: the endpoint may not ship without the ingress-restriction
//! guidance deployers copy from.

#[cfg(test)]
mod tests {
    const README: &str = include_str!("../../../README.md");
    const RESTRICT_DOC: &str =
        include_str!("../../../docs/deployment/restrict-operational-endpoints.md");

    #[test]
    fn health_gvl_ingress_restriction_documented_in_readme() {
        for required in [
            "## Operational endpoints (`/_auctioneer/*`)",
            "`GET /_auctioneer/health/gvl`",
            "fail-closed",
            "[gvl.authenticated_health]",
            "token_secret",
            "restrict the `/_auctioneer/*` path prefix at the ingress layer",
            "docs/deployment/restrict-operational-endpoints.md",
        ] {
            assert!(
                README.contains(required),
                "README.md must carry the /_auctioneer/* restriction guidance; missing: {required}"
            );
        }
        // The linked recipe doc must carry an L7 recipe per adapter —
        // and no L3/L4 NetworkPolicy recipe (it cannot match an HTTP
        // path).
        for required in [
            "## Cloudflare Workers",
            "## Fastly Compute@Edge",
            "## Kubernetes",
            "## Standalone Axum",
            "auth_request",
        ] {
            assert!(
                RESTRICT_DOC.contains(required),
                "restrict-operational-endpoints.md must carry every per-platform L7 recipe; missing: {required}"
            );
        }
        assert!(
            !RESTRICT_DOC.contains("kind: NetworkPolicy"),
            "restrict-operational-endpoints.md must not recommend an L3/L4 NetworkPolicy for path restriction"
        );
    }
}
