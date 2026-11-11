# Restricting the `/_auctioneer/*` prefix at the ingress

auctioneer serves operational endpoints under the `/_auctioneer/*` prefix
(`/_auctioneer/health/gvl` today). The shared handler resolves
`host.gvl` from the pushed operator policy on every health request. An omitted
section or `authenticate_health` mode keeps health closed.
`authenticate_health = true` requires a nonempty
`authenticate_health_token_secret` entry name in the default Secret Store; an
empty reference is invalid. Missing/malformed config always denies access.
In authenticated mode, secret faults, empty secrets, secrets over 4096 bytes and failed authorization
return 401 with a Bearer challenge. Reads use the existing config-fetch deadline;
provider APIs may materialize a complete secret before the accepted-size check,
and synchronous provider calls cannot be preempted by a cooperative deadline.
`/status` remains independent.

To allow anonymous access, explicitly configure:

```toml
[host.gvl]
authenticate_health = false
```

This mode requires the secret reference to be omitted and never reads a secret.
An empty or named reference alongside false is invalid. Authenticated references
must be at most 128 UTF-8 bytes, without ASCII controls or surrounding whitespace. A
reference without a mode, unknown fields and explicit nulls are rejected.
Anonymous access requires an ingress restriction for the prefix. Ingress
restriction is recommended defence in depth for authenticated
health as well. Adapters install counter scope only; provider flags and boot
credentials do not determine access. See the
[secret provisioning mappings](../../README.md#operational-endpoints-_auctioneer).

Path restriction is an **L7 concern**: the rule must match the HTTP
path. An L3/L4 mechanism (IP + port), such as a Kubernetes
`NetworkPolicy`, cannot see the path and does NOT satisfy this
requirement.

## Cloudflare Workers

Create a [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/policies/access/)
application over the internal prefix so requests authenticate at
Cloudflare's edge before reaching the Worker:

```
Application domain: <your-worker-domain>
Path:               /_auctioneer/*
Policy:             Allow — require your operator identity group
```

Terraform sketch:

```hcl
resource "cloudflare_zero_trust_access_application" "auctioneer_internal" {
  zone_id = var.zone_id
  name    = "auctioneer internal endpoints"
  domain  = "${var.auctioneer_host}/_auctioneer/*"
}
```

## Fastly Compute@Edge

Attach VCL (or the equivalent Compute request guard) that rejects
unauthorised requests to the prefix before they reach the service:

A presence check is **not** authentication: `Authorization: junk`
satisfies `if (!req.http.Authorization)` and the shared handler permits
anonymous access when `host.gvl.authenticate_health = false`. The value has
to be VALIDATED, and the comparison has to be constant-time so the
token cannot be recovered byte by byte from response timing.

Store the expected credential in an edge dictionary rather than
inlining it in VCL, and compare with `digest.secure_is_equal`:

```vcl
# Edge dictionary `auctioneer_ingress` holds key `health_token`.
# Create it with:
#   fastly dictionary create --name auctioneer_ingress --version latest
#   fastly dictionary-item create --dictionary-id <id> \
#     --key health_token --value "<a long random secret>"
table auctioneer_ingress {}

if (req.url.path ~ "^/_auctioneer/") {
  declare local var.expected STRING;
  declare local var.presented STRING;
  set var.expected = table.lookup(auctioneer_ingress, "health_token", "");
  set var.presented = req.http.Authorization;

  # An unset dictionary key must FAIL CLOSED: an empty expected value
  # would otherwise match a request that presented "Bearer ".
  if (var.expected == "") {
    error 401 "unauthorized";
  }
  # `digest.secure_is_equal` is constant-time; `==` on a secret is not.
  if (!digest.secure_is_equal(var.presented, "Bearer " var.expected)) {
    error 401 "unauthorized";
  }
}
```

For authenticated health, use the same credential selected by
`host.gvl.authenticate_health_token_secret` in the linked default Fastly secret
store (for example `health_gvl_token`). Keeping the two in step means a
rotation that misses one layer fails closed rather than open.

Alternatively, and preferably where it is available, scope the service
behind a signed-request policy or Fastly's own edge authentication for
the prefix, so no credential is compared in VCL at all.

## Kubernetes (any ingress-fronted deployment)

Use an L7 proxy — nginx-ingress or istio — with a route matching the
prefix and an authorization policy. A `NetworkPolicy` alone is NOT
sufficient (L3/L4 only).

istio example (`HTTPRoute` match + `AuthorizationPolicy`):

```yaml
apiVersion: security.istio.io/v1
kind: AuthorizationPolicy
metadata:
  name: auctioneer-deny-internal
spec:
  selector:
    matchLabels:
      app: auctioneer
  action: DENY
  rules:
    - to:
        - operation:
            paths: ["/_auctioneer/*"]
      from:
        - source:
            notNamespaces: ["ops"]
```

nginx-ingress equivalent: a dedicated `Ingress` for `/_auctioneer/` with
`nginx.ingress.kubernetes.io/auth-url` pointing at your auth service.

## Standalone Axum

Run behind an ingress that terminates auth for the prefix — nginx
`auth_request` (or envoy `ext_authz`):

```nginx
location /_auctioneer/ {
  auth_request /_oauth2/auth;
  proxy_pass   http://auctioneer;
}
```
