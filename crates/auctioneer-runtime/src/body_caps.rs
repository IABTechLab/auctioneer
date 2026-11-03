//! Per-route body-cap accessor for the `EdgeZero` ingress hook.
//!
//! The `RouteMatch` shape carries the ingress hook contract.
//!
//! The hook calls [`match_route`] with the raw request method + path and
//! the application's `RouteTable`; the returned [`RouteMatch`] carries the
//! body cap the hook applies. A path that doesn't match any wired route
//! returns `NotFound { cap_bytes: 4096 }`; a known path with the wrong
//! method returns `MethodNotAllowed { cap_bytes: 4096 }`.

use http::Method;
use std::sync::OnceLock;

/// Default body cap (4 KiB) applied to non-body routes, unmatched paths,
/// and wrong-method dispatches.
pub const DEFAULT_CAP_BYTES: usize = 4_096;

/// 256 KiB cap for `POST /openrtb2/auction` (`MAX_AUCTION_BODY`).
pub const AUCTION_CAP_BYTES: usize = 256 * 1024;

/// 32 KiB cap for `POST /cookie_sync` (`MAX_COOKIESYNC_BODY`).
pub const COOKIESYNC_CAP_BYTES: usize = 32 * 1024;

/// 4 KiB cap for `GET /_auctioneer/health/gvl` — the no-body route class.
pub const HEALTH_GVL_CAP_BYTES: usize = 4_096;

/// One route registered by the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// Stable identifier (`"auction"`, `"cookie_sync"`, `"status"`,
    /// `"info_bidders"`, `"info_bidder"`, `"setuid"`, `"getuids"`).
    pub id: &'static str,
    /// HTTP method this route is wired under.
    pub method: Method,
    /// Exact path or path template (`/info/bidders/{bidder}`).
    pub path: &'static str,
    /// Body cap in bytes.
    pub cap_bytes: usize,
}

/// Application-scoped set of wired routes. Constructed from
/// `edgezero.toml` registrations once per build; the ingress hook
/// borrows it for every dispatch.
#[derive(Clone, Debug)]
pub struct RouteTable {
    routes: Vec<Route>,
}

impl RouteTable {
    /// Build a `RouteTable` from an explicit slice of routes. The
    /// caller is responsible for matching the slice against the
    /// application's actual `[[triggers.http]]` registrations.
    #[must_use]
    #[inline]
    pub fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }

    /// Empty route table — every `match_route` call returns `NotFound`
    /// with the default cap. Useful for tests and applications with
    /// no body-bearing routes wired yet.
    #[must_use]
    #[inline]
    pub fn empty() -> Self {
        Self { routes: Vec::new() }
    }

    /// Iterate over the registered routes.
    #[must_use]
    #[inline]
    pub fn routes(&self) -> &[Route] {
        &self.routes
    }
}

impl Default for RouteTable {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

/// The application's wired routes — one entry per
/// `[[triggers.http]]` registration in the workspace `edgezero.toml`.
/// The ingress hook and the handler-side registration test read THIS
/// table, and `tests/route_table_parity.rs` asserts a bijection with
/// the manifest on `(id, method, path)`, so the two hand-maintained
/// lists cannot drift silently (a drifted table would 404 a wired
/// route, apply the wrong cap, or select the wrong rejection body).
#[must_use]
pub fn table() -> &'static RouteTable {
    static TABLE: OnceLock<RouteTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        RouteTable::new(vec![
            Route {
                id: "auction",
                method: Method::POST,
                path: "/openrtb2/auction",
                cap_bytes: AUCTION_CAP_BYTES,
            },
            Route {
                id: "status",
                method: Method::GET,
                path: "/status",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "info_bidders",
                method: Method::GET,
                path: "/info/bidders",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "info_bidder",
                method: Method::GET,
                path: "/info/bidders/{name}",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "health_gvl",
                method: Method::GET,
                path: "/_auctioneer/health/gvl",
                cap_bytes: HEALTH_GVL_CAP_BYTES,
            },
            Route {
                id: "cookie_sync",
                method: Method::POST,
                path: "/cookie_sync",
                cap_bytes: COOKIESYNC_CAP_BYTES,
            },
            Route {
                id: "cookie_sync_preflight",
                method: Method::OPTIONS,
                path: "/cookie_sync",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "setuid",
                method: Method::GET,
                path: "/setuid",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "getuids",
                method: Method::GET,
                path: "/getuids",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
        ])
    })
}

/// Outcome of route classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteMatch {
    /// Path + method match a wired route. `cap_bytes` is the route's
    /// per-route cap (256 KiB for `auction`, 32 KiB for `cookie_sync`,
    /// 4 KiB for non-body routes).
    Matched {
        route_id: &'static str,
        cap_bytes: usize,
    },
    /// Path matches a wired route but the method does not. The hook
    /// applies the default 4 KiB cap and dispatch returns 405.
    MethodNotAllowed { cap_bytes: usize },
    /// No wired route matches the path. The hook applies the default
    /// 4 KiB cap and dispatch returns 404.
    NotFound { cap_bytes: usize },
}

/// Classify a request against the application's wired routes. The hook
/// calls this first, then runs Content-Encoding → Content-Length →
/// bounded-read checks against the returned `cap_bytes`.
///
/// Path matching supports literal exact-match plus a single
/// `{name}` trailing segment placeholder (used by
/// `/info/bidders/{bidder}`). Wildcards or multi-segment templates
/// are out of scope here — adapters that need them register
/// distinct routes for the alternatives.
#[must_use]
#[inline]
pub fn match_route(method: &Method, path: &str, active_routes: &RouteTable) -> RouteMatch {
    let mut path_matched = false;
    for route in active_routes.routes() {
        if !path_matches(route.path, path) {
            continue;
        }
        path_matched = true;
        if route.method == *method {
            return RouteMatch::Matched {
                route_id: route.id,
                cap_bytes: route.cap_bytes,
            };
        }
    }
    if path_matched {
        return RouteMatch::MethodNotAllowed {
            cap_bytes: DEFAULT_CAP_BYTES,
        };
    }
    RouteMatch::NotFound {
        cap_bytes: DEFAULT_CAP_BYTES,
    }
}

/// Literal-or-`{name}` path match. Strict byte-exact split: a trailing
/// slash on the incoming path is treated as an extra empty segment, so
/// `/openrtb2/auction/` does NOT match the `/openrtb2/auction` route
/// (it falls through to `NotFound { cap_bytes: 4096 }`).
///
/// The hook contract says the hook receives the raw path; any
/// normalization (path canonicalization, trailing-slash folding)
/// is the framework's job done BEFORE both routing and cap lookup.
/// Doing it here too would silently widen the cap surface to any
/// "/route/" variant the framework didn't normalize, so we don't.
fn path_matches(template: &str, incoming: &str) -> bool {
    let template_segs: Vec<&str> = template.split('/').collect();
    let incoming_segs: Vec<&str> = incoming.split('/').collect();
    if template_segs.len() != incoming_segs.len() {
        return false;
    }
    template_segs
        .iter()
        .zip(incoming_segs.iter())
        .all(|(tpl, inc)| {
            if tpl.starts_with('{') && tpl.ends_with('}') {
                !inc.is_empty()
            } else {
                tpl == inc
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routes_without_sync() -> RouteTable {
        RouteTable::new(vec![
            Route {
                id: "auction",
                method: Method::POST,
                path: "/openrtb2/auction",
                cap_bytes: AUCTION_CAP_BYTES,
            },
            Route {
                id: "status",
                method: Method::GET,
                path: "/status",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "info_bidders",
                method: Method::GET,
                path: "/info/bidders",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
            Route {
                id: "info_bidder",
                method: Method::GET,
                path: "/info/bidders/{bidder}",
                cap_bytes: DEFAULT_CAP_BYTES,
            },
        ])
    }

    #[test]
    fn auction_post_matches_with_256k_cap() {
        let routes = routes_without_sync();
        let result = match_route(&Method::POST, "/openrtb2/auction", &routes);
        assert_eq!(
            result,
            RouteMatch::Matched {
                route_id: "auction",
                cap_bytes: AUCTION_CAP_BYTES,
            }
        );
    }

    #[test]
    fn auction_get_returns_method_not_allowed_with_default_cap() {
        let routes = routes_without_sync();
        let result = match_route(&Method::GET, "/openrtb2/auction", &routes);
        assert_eq!(
            result,
            RouteMatch::MethodNotAllowed {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn unregistered_cookie_sync_route_returns_not_found() {
        let routes = routes_without_sync();
        let result = match_route(&Method::POST, "/cookie_sync", &routes);
        assert_eq!(
            result,
            RouteMatch::NotFound {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn registered_cookie_sync_route_uses_32k_cap() {
        let routes = RouteTable::new(vec![Route {
            id: "cookie_sync",
            method: Method::POST,
            path: "/cookie_sync",
            cap_bytes: COOKIESYNC_CAP_BYTES,
        }]);
        let result = match_route(&Method::POST, "/cookie_sync", &routes);
        assert_eq!(
            result,
            RouteMatch::Matched {
                route_id: "cookie_sync",
                cap_bytes: COOKIESYNC_CAP_BYTES,
            }
        );
    }

    #[test]
    fn empty_table_returns_not_found_with_default_cap() {
        let result = match_route(&Method::GET, "/anything", &RouteTable::empty());
        assert_eq!(
            result,
            RouteMatch::NotFound {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn info_bidder_template_matches_concrete_path() {
        let routes = routes_without_sync();
        let result = match_route(&Method::GET, "/info/bidders/fake_bidder", &routes);
        assert_eq!(
            result,
            RouteMatch::Matched {
                route_id: "info_bidder",
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn auction_with_trailing_slash_returns_not_found() {
        // A trim_matches('/') would make POST /openrtb2/auction/ match
        // the auction route and grant the 256 KiB cap. Path matching
        // is raw; the trailing-slash variant must fall through to
        // NotFound (4 KiB cap).
        let routes = routes_without_sync();
        let result = match_route(&Method::POST, "/openrtb2/auction/", &routes);
        assert_eq!(
            result,
            RouteMatch::NotFound {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn auction_without_leading_slash_returns_not_found() {
        // Symmetric guard: openrtb2/auction (no leading /) must not
        // match the /openrtb2/auction route either.
        let routes = routes_without_sync();
        let result = match_route(&Method::POST, "openrtb2/auction", &routes);
        assert_eq!(
            result,
            RouteMatch::NotFound {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn info_bidder_template_does_not_match_extra_segment() {
        let routes = routes_without_sync();
        // Template /info/bidders/{bidder} has three segments; an incoming
        // path with four segments must NOT match it (no multi-segment
        // wildcard support).
        let result = match_route(&Method::GET, "/info/bidders/fake_bidder/extra", &routes);
        assert_eq!(
            result,
            RouteMatch::NotFound {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn status_matches_with_default_cap() {
        let routes = routes_without_sync();
        let result = match_route(&Method::GET, "/status", &routes);
        assert_eq!(
            result,
            RouteMatch::Matched {
                route_id: "status",
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }

    #[test]
    fn unmatched_path_returns_not_found() {
        let routes = routes_without_sync();
        let result = match_route(&Method::POST, "/missing", &routes);
        assert_eq!(
            result,
            RouteMatch::NotFound {
                cap_bytes: DEFAULT_CAP_BYTES,
            }
        );
    }
}
