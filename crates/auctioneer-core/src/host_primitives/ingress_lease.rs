//! The one value auctioneer puts in a host ingress grant, and the
//! route-class permit that rides the response's late-bound slot.
//!
//! # Why a permit must NOT live in the grant
//!
//! The host router drops the per-request grant BEFORE it constructs a
//! fallback response — the canonical 404/405, a body-overflow rejection,
//! a read timeout. Verified in the router's fallback drain: `drop(grant)`
//! precedes the `match` that builds the response. A releasable resource
//! stored in the grant is therefore freed while its response is still
//! unwritten, which OVER-ADMITS: the next request acquires the slot the
//! unwritten response is still going to occupy.
//!
//! So the lease carries a zero-sized MARKER and nothing releasable. The
//! permit itself is moved into the [`ResponseEgressCompletion`] at
//! admission, where the host owns it until terminal egress — and that
//! covers every response on the route, handler-served or
//! framework-generated, because the completion travels with the
//! response rather than with the grant.
//!
//! # Why one grant payload and not two
//!
//! An ingress grant carries ONE type-erased value, so auctioneer defines
//! exactly one: [`AuctioneerIngressLease`]. It holds the route-class marker
//! and the ONE late-bound resource handle. The slot's occupant is
//! decided by ROUTE CLASS — an auction route installs an auction
//! permit, an aux route installs an aux permit — and a request is never
//! both, so one slot suffices and no request can hold two class permits
//! at once.
//!
//! The lease also carries the auction-concurrency slot, and that is the
//! one place a releasable resource DOES ride the grant. It is not an
//! exception to the rule above; it is the rule applied. The rule is
//! "nothing whose release must wait for the response" — and an
//! auction-concurrency slot bounds the auction's WORK, which is over
//! when the handler is. Releasing it at grant drop is therefore on time,
//! not early, and it is exactly the boundary the recovery path wants: a
//! request whose config read expired returns its auction slot promptly
//! rather than holding it through the refusal's transmission. Every
//! TRANSMISSION bound still lives in the completion.
//!
//! # Why the permits are owned rather than borrowed
//!
//! A grant value must be `Send + Sync + 'static` and a late-bound
//! completion value must be `Send + 'static`. Anything holding a
//! reference into gate state satisfies neither. That is pinned at
//! COMPILE time below rather than in prose, because a lifetime leaking
//! back in is a type error a prose rule would not catch.

use auctioneer_runtime::{AdmissionPermit, AuxPermit, EgressPermit, StaticEgressPermit};
use edgezero_core::response_egress::{
    ResponseEgressCompletion, ResponseEgressResource, ResponseEgressResourceInstallError,
};

/// Which gate admitted this request, and nothing more.
///
/// Zero-sized on every variant on purpose: it exists so a handler can
/// ASSERT it was admitted and read back which class admitted it, not so
/// it can release anything. Dropping the grant that carries it releases
/// no capacity, which is the whole point — see the module docs on the
/// router's `drop(grant)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteAdmissionMarker {
    /// Admitted by the auction admission gate.
    Auction,
    /// Admitted by the aux-route gate. Covers the pre-resolved 404/405
    /// fallback routes too: they are small capped reads, and spending
    /// auction capacity on a 404 is not a trade auctioneer makes.
    Aux,
    /// Admitted by the independent liveness-output gate.
    Probe,
}

impl RouteAdmissionMarker {
    /// The class name, for logs and diagnostics.
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::Auction => "auction",
            Self::Aux => "aux",
            Self::Probe => "probe",
        }
    }
}

/// The route-class permit a response holds until terminal egress.
///
/// One enum rather than a second late-bound pair, because a second pair
/// would need a second completion and only one completion fits in an
/// admission decision. A request belongs to exactly one route class, so
/// exactly one variant is ever installed.
#[derive(Debug)]
pub enum RouteEgressPermit {
    /// An auction response's egress-class permit.
    Auction(EgressPermit),
    /// An aux route's permit, including the fallback routes.
    Aux(AuxPermit),
    /// A probe output's reserved permit, independent of auction egress.
    Probe(EgressPermit),
}

impl RouteEgressPermit {
    /// The class this permit was drawn from.
    #[must_use]
    pub const fn marker(&self) -> RouteAdmissionMarker {
        match *self {
            Self::Auction(_) => RouteAdmissionMarker::Auction,
            Self::Aux(_) => RouteAdmissionMarker::Aux,
            Self::Probe(_) => RouteAdmissionMarker::Probe,
        }
    }
}

/// The ONLY type auctioneer puts in a host ingress grant.
///
/// Built during synchronous admission, alongside the completion the same
/// admission decision carries. The handler takes it back out of the
/// request context and installs its route-class permit into
/// [`route_egress_resource`](Self::route_egress_resource).
#[derive(Debug)]
pub struct AuctioneerIngressLease {
    /// Which gate admitted this request. A marker, never a permit.
    pub admission: RouteAdmissionMarker,
    /// The late-bound slot the route's permit is installed into. Its
    /// occupant is decided by route class; the host closes it at
    /// terminal egress and drops the permit exactly once.
    pub route_egress_resource: ResponseEgressResource<RouteEgressPermit>,
    /// The auction-concurrency slot, for an auction admission.
    ///
    /// This is the ONE resource that belongs in the grant rather than in
    /// the completion, and the distinction is the whole reason the field
    /// exists rather than being folded into the completion with the
    /// egress permit. An EGRESS permit bounds TRANSMISSION, so it must
    /// outlive the grant — the router drops the grant before it builds a
    /// fallback response, and a transmission bound released there would
    /// over-admit. This permit bounds the auction's WORK, which is over
    /// when the handler is, so releasing it at grant drop is not
    /// early — it is exactly when the thing it bounds has finished.
    ///
    /// `None` for every aux and fallback admission: the aux class is
    /// bounded by the permit in the completion and takes no
    /// auction-concurrency slot.
    pub auction_admission: Option<AdmissionPermit>,
}

/// The host trait bounds these types must satisfy, checked at compile
/// time.
///
/// A borrowing permit — anything holding a reference into gate state —
/// fails these bounds and this stops compiling. That is deliberately
/// stronger than a comment: prose cannot catch a lifetime parameter
/// leaking back into one of these types, and a type error can.
const _: fn() = || {
    fn assert_grant<T: Send + Sync + 'static>() {}
    fn assert_resource<T: Send + 'static>() {}
    assert_grant::<AuctioneerIngressLease>();
    assert_resource::<RouteEgressPermit>();
};

/// Build the admission-time lease and its paired completion for a
/// route whose permit is ALREADY in hand.
///
/// Returns the value the ingress grant carries and the completion the
/// admission decision carries. `permit` is moved INTO the completion
/// immediately, so the host owns it from this instant: every terminal
/// path releases it, including the fallback responses built after the
/// grant is dropped, an admission refusal that has no grant at all, and
/// plain abandonment.
///
/// This is the AUX shape. An aux route's class permit is acquired once,
/// at admission, by [`AuxGate`](auctioneer_runtime::AuxGate) — the route is
/// cheap and its whole cost is bounded before the body is read — so
/// there is a permit to bind at this point. Admission acquires ONCE: a
/// handler that acquired a second permit from its own class's gate
/// would, at capacity one, block on a slot it is itself holding, which
/// is an admitted request rejecting itself.
///
/// An AUCTION route uses [`admit_pending`] instead, because its egress
/// permit does not exist yet.
#[must_use]
pub fn admit(
    marker: RouteAdmissionMarker,
    permit: RouteEgressPermit,
) -> (AuctioneerIngressLease, ResponseEgressCompletion) {
    let (lease, completion) = admit_pending(marker);
    // A fresh slot cannot already be occupied or closed, so this
    // install cannot fail — but the result is consumed explicitly
    // rather than discarded, because "cannot fail" is an argument about
    // today's code and the logging inside covers it if that changes.
    let installed = install_route_permit(&lease.route_egress_resource, permit);
    debug_assert!(installed, "a freshly created slot accepts its permit");
    (lease, completion)
}

/// Build the admission-time lease and its paired completion for a route
/// whose permit does NOT exist yet.
///
/// This is the AUCTION shape. The egress permit bounds TRANSMISSION,
/// and until the auction has resolved there is nothing to transmit, so
/// acquiring one at admission would hold egress capacity for the whole
/// duration of a fan-out that has not produced a byte. The handler
/// acquires after `run_auction` and calls [`install_route_permit`] on
/// the slot this returns.
///
/// The slot may legitimately stay EMPTY: a refusal, a config failure or
/// a timeout means the handler never acquires. The completion then
/// simply closes an empty slot, which is a no-op — there is no path
/// where an uninstalled slot strands capacity.
#[must_use]
pub fn admit_pending(
    marker: RouteAdmissionMarker,
) -> (AuctioneerIngressLease, ResponseEgressCompletion) {
    admit_pending_holding(marker, None)
}

/// [`admit_pending`], with an auction-concurrency slot held in the
/// lease.
///
/// The auction-concurrency permit is the one resource that rides the
/// GRANT: it bounds the auction's work rather than the response's
/// transmission, so releasing it when the grant drops releases it when
/// the work is over. See
/// [`auction_admission`](AuctioneerIngressLease::auction_admission) for why
/// that is not the over-admission the egress permit would cause.
#[must_use]
pub fn admit_pending_holding(
    marker: RouteAdmissionMarker,
    auction_admission: Option<AdmissionPermit>,
) -> (AuctioneerIngressLease, ResponseEgressCompletion) {
    admit_pending_holding_terminal(marker, auction_admission, None)
}

/// [`admit_pending_holding`], with the request's TERMINAL-response slot
/// reserved at admission and joined to the route resource's close.
///
/// Every admitted request eventually puts one response on the wire,
/// whatever produces it. The route-class egress permit does not cover
/// that: it is acquired by the handler only once an auction has
/// resolved, so an admitted request that answers with a config error, a
/// validation error or any other early response installs nothing at all
/// and its response would travel out accounted for by nothing.
///
/// Reserving the terminal slot HERE closes that gap, and it is the same
/// class a pre-admission rejection and an admission refusal already
/// draw from, so "every response auctioneer puts on the wire holds a
/// terminal permit" holds without a route-shaped exception.
///
/// The permit is moved into the COMPLETION rather than into the lease,
/// for the reason the module docs give: the router drops the grant
/// before it builds a fallback response, and a transmission bound
/// released there would over-admit. Its release is joined with the
/// route resource's close so one completion covers both, because an
/// admission decision carries exactly one.
#[must_use]
pub fn admit_pending_holding_terminal(
    marker: RouteAdmissionMarker,
    auction_admission: Option<AdmissionPermit>,
    terminal: Option<StaticEgressPermit>,
) -> (AuctioneerIngressLease, ResponseEgressCompletion) {
    let (route_egress_resource, route_completion) =
        ResponseEgressCompletion::late_bound::<RouteEgressPermit>();
    // One completion, two owned resources. `join` is the host's own
    // primitive for this: both halves run at most once at terminal
    // egress, and ABANDONING the joined completion releases each
    // exactly once — which is the cancellation path, where a leak would
    // be permanent because no terminal report ever arrives.
    let completion = match terminal {
        Some(permit) => {
            route_completion.join(ResponseEgressCompletion::new(move |_report| drop(permit)))
        }
        // No terminal slot to join: the caller either does not reserve
        // one for this class, or the class is exhausted and the caller
        // has already decided what to do about that.
        None => route_completion,
    };
    (
        AuctioneerIngressLease {
            admission: marker,
            route_egress_resource,
            auction_admission,
        },
        completion,
    )
}

/// Install a route-class permit into its response's late-bound slot,
/// logging the two ways that can legitimately fail.
///
/// Returns whether the permit is now owned by the completion. `false`
/// means the permit was dropped inside this call — which releases its
/// slot, so nothing leaks either way.
///
/// The two failures are NOT handler bugs in equal measure:
///
/// - `Closed` — terminal egress already happened, or the completion was
///   abandoned. The response is already gone, so there is nothing to
///   hold capacity for. Logged at debug and treated as normal, because
///   a client that disconnects mid-auction produces exactly this and it
///   is not a defect.
/// - `AlreadyInstalled` — something installed twice. That IS a bug, but
///   not a fatal one: the second permit drops and releases, so the gate
///   accounting stays correct. Logged at error so it is visible, and it
///   does not panic.
#[must_use]
pub fn install_route_permit(
    resource: &ResponseEgressResource<RouteEgressPermit>,
    permit: RouteEgressPermit,
) -> bool {
    let class = permit.marker().class();
    match resource.install(permit) {
        Ok(()) => true,
        Err(ResponseEgressResourceInstallError::Closed) => {
            log::debug!(
                "{class} egress permit released immediately: the response reached terminal egress \
                 before the permit was installed"
            );
            false
        }
        Err(ResponseEgressResourceInstallError::AlreadyInstalled) => {
            log::error!(
                "{class} egress permit installed twice for one response; the duplicate was \
                 dropped and its capacity returned"
            );
            false
        }
        // The host's rejection reasons are open, so a future one lands
        // here. Treating it as a refusal is the safe reading either
        // way: the host has already dropped the permit it refused, so
        // the slot is back, and reporting `false` is honest about
        // auctioneer no longer owning it.
        Err(unknown) => {
            log::error!("{class} egress permit rejected by the host: {unknown}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuctioneerIngressLease, RouteAdmissionMarker, RouteEgressPermit, admit, admit_pending,
        admit_pending_holding, install_route_permit,
    };
    use auctioneer_runtime::{AdmissionGate, AuxGate, EgressGate};
    use edgezero_core::response_egress::ResponseEgressCompletion;

    /// Admission binds the permit to the COMPLETION, not to the lease,
    /// so dropping the grant the way the router's fallback path does
    /// releases nothing. This is the over-admission bug in test form: if
    /// the permit rode the lease, the gate would free a slot here while
    /// the response was still unwritten.
    #[test]
    fn dropping_the_lease_releases_no_capacity() {
        let gate = AuxGate::new(1);
        let permit = gate.try_acquire().expect("capacity 1 admits the first");
        let (lease, completion) = admit(RouteAdmissionMarker::Aux, RouteEgressPermit::Aux(permit));
        assert_eq!(gate.in_use(), 1, "admission holds the slot");

        // Exactly what the router does before building a fallback
        // response.
        drop(lease);
        assert_eq!(
            gate.in_use(),
            1,
            "dropping the grant must NOT release the permit: the response is still unwritten",
        );
        assert!(
            gate.try_acquire().is_none(),
            "a second aux request cannot acquire while the first response is in flight",
        );

        // Only the completion's own close releases it. Abandoning the
        // completion is the host's drop path, and it releases exactly
        // once.
        drop(completion);
        assert_eq!(
            gate.in_use(),
            0,
            "the completion owns the permit and releases it when it closes",
        );
        assert!(gate.try_acquire().is_some(), "the slot is reusable");
    }

    /// A permit installed after the completion closed drops
    /// immediately, so the gate recovers its slot rather than leaking
    /// it. This is the client-disconnected-mid-auction shape, and it
    /// must not be treated as a handler bug.
    #[test]
    fn install_after_close_drops_permit_immediately() {
        let gate = EgressGate::new(1);
        let (resource, completion) = ResponseEgressCompletion::late_bound::<RouteEgressPermit>();
        // Close the slot before anything is installed.
        drop(completion);

        let permit = gate.try_acquire().expect("capacity 1 admits");
        assert_eq!(gate.in_use(), 1);
        let installed = install_route_permit(&resource, RouteEgressPermit::Auction(permit));
        assert!(!installed, "a closed slot does not take ownership");
        assert_eq!(
            gate.in_use(),
            0,
            "the rejected permit was dropped inside the install, releasing its slot",
        );
    }

    /// A duplicate install is an error, not a leak: the second permit
    /// drops and returns its slot, no panic.
    #[test]
    fn duplicate_install_is_an_error_not_a_leak() {
        let gate = EgressGate::new(2);
        let (resource, completion) = ResponseEgressCompletion::late_bound::<RouteEgressPermit>();

        let first = gate.try_acquire().expect("slot one");
        let second = gate.try_acquire().expect("slot two");
        assert_eq!(gate.in_use(), 2);

        assert!(
            install_route_permit(&resource, RouteEgressPermit::Auction(first)),
            "the first install takes ownership",
        );
        assert_eq!(gate.in_use(), 2, "the installed permit is still held");

        assert!(
            !install_route_permit(&resource, RouteEgressPermit::Auction(second)),
            "the second install is rejected",
        );
        assert_eq!(
            gate.in_use(),
            1,
            "the duplicate released its own slot and left the installed one alone",
        );

        drop(completion);
        assert_eq!(
            gate.in_use(),
            0,
            "closing releases the installed permit once"
        );
    }

    /// Terminal-callback and abandonment both release, and they release
    /// EXACTLY once. The host runs the completion at most once, so this
    /// asserts auctioneer's side: one permit in, one slot back.
    #[test]
    fn every_terminal_path_releases_exactly_once() {
        let gate = EgressGate::new(1);
        let (resource, completion) = ResponseEgressCompletion::late_bound::<RouteEgressPermit>();
        let permit = gate.try_acquire().expect("capacity 1 admits");
        assert!(install_route_permit(
            &resource,
            RouteEgressPermit::Auction(permit)
        ));
        assert_eq!(gate.in_use(), 1);

        drop(completion);
        assert_eq!(gate.in_use(), 0, "the close released the permit");
        // The installer outlives the completion by design; using it
        // again must not double-release.
        let extra = gate.try_acquire().expect("the slot came back");
        assert!(!install_route_permit(
            &resource,
            RouteEgressPermit::Auction(extra)
        ));
        assert_eq!(
            gate.in_use(),
            0,
            "a post-close install releases its own permit and nothing else",
        );
    }

    /// One slot, one class: an aux route and an auction route cannot
    /// both occupy a response's late-bound slot, because there is one
    /// slot and a request has one class.
    #[test]
    fn aux_and_auction_permits_are_mutually_exclusive() {
        let aux = AuxGate::new(1);
        let egress = EgressGate::new(1);
        let (resource, completion) = ResponseEgressCompletion::late_bound::<RouteEgressPermit>();

        let aux_permit = aux.try_acquire().expect("aux capacity 1");
        assert!(install_route_permit(
            &resource,
            RouteEgressPermit::Aux(aux_permit)
        ));

        let auction_permit = egress.try_acquire().expect("egress capacity 1");
        assert!(
            !install_route_permit(&resource, RouteEgressPermit::Auction(auction_permit)),
            "the slot is taken by the aux permit; the auction permit is refused",
        );
        assert_eq!(
            egress.in_use(),
            0,
            "the refused auction permit released its own slot",
        );
        assert_eq!(aux.in_use(), 1, "the aux permit is still installed");

        drop(completion);
        assert_eq!(aux.in_use(), 0);
    }

    /// The marker round-trips its class and carries no payload beyond
    /// the discriminant, so it cannot be holding anything releasable.
    ///
    /// A two-variant enum is one byte, not zero — the plan's
    /// "zero-sized" wording is about the PAYLOAD, and that is what is
    /// asserted: the marker is `Copy` and exactly discriminant-sized,
    /// which a variant carrying a permit could not be.
    #[test]
    fn the_admission_marker_carries_no_releasable_payload() {
        // `Copy` is the structural proof: a type owning a permit could
        // not be `Copy`, because releasing on drop and duplicating
        // freely are incompatible.
        const fn assert_copy<T: Copy>() {}
        assert_eq!(
            size_of::<RouteAdmissionMarker>(),
            1,
            "the marker is a bare discriminant: no payload to release",
        );
        assert_copy::<RouteAdmissionMarker>();
        assert_eq!(RouteAdmissionMarker::Auction.class(), "auction");
        assert_eq!(RouteAdmissionMarker::Aux.class(), "aux");
    }

    /// The auction shape: admission hands back an EMPTY slot, the
    /// handler fills it later, and the completion releases what it
    /// finds. Acquiring at admission instead would hold egress capacity
    /// for the whole fan-out, before there is anything to transmit.
    #[test]
    fn a_pending_admission_takes_no_capacity_until_the_handler_installs() {
        let gate = EgressGate::new(1);
        let (lease, completion) = admit_pending(RouteAdmissionMarker::Auction);
        assert_eq!(
            gate.in_use(),
            0,
            "a pending admission holds no egress capacity: nothing is transmittable yet",
        );

        // The handler acquires after its work resolves.
        let permit = gate.try_acquire().expect("capacity 1 admits");
        assert!(install_route_permit(
            &lease.route_egress_resource,
            RouteEgressPermit::Auction(permit)
        ));
        assert_eq!(gate.in_use(), 1, "the installed permit holds its slot");

        drop(lease);
        assert_eq!(
            gate.in_use(),
            1,
            "dropping the grant does not release what the completion owns",
        );
        drop(completion);
        assert_eq!(gate.in_use(), 0, "terminal close releases it exactly once");
    }

    /// The auction-concurrency slot releases when the GRANT drops, and
    /// the egress slot does not. Both halves are asserted together,
    /// because the whole point of the two living in one lease is that
    /// they release at DIFFERENT boundaries: the concurrency slot bounds
    /// work that is over when the handler is, and the egress slot bounds
    /// a transmission that has not happened yet.
    #[test]
    fn the_concurrency_slot_releases_with_the_grant_and_the_egress_slot_does_not() {
        let auctions = AdmissionGate::new(1);
        let egress = EgressGate::new(1);
        let concurrency = auctions.try_acquire().expect("capacity 1 admits");
        let (lease, completion) =
            admit_pending_holding(RouteAdmissionMarker::Auction, Some(concurrency));
        assert_eq!(auctions.in_use(), 1, "admission holds the concurrency slot");

        // The handler acquires its egress permit after the auction
        // resolves and installs it into the same lease's slot.
        let permit = egress.try_acquire().expect("capacity 1 admits");
        assert!(install_route_permit(
            &lease.route_egress_resource,
            RouteEgressPermit::Auction(permit)
        ));

        drop(lease);
        assert_eq!(
            auctions.in_use(),
            0,
            "the concurrency slot returns with the grant: the auction's work is over",
        );
        assert_eq!(
            egress.in_use(),
            1,
            "the egress permit does NOT: its response is still unwritten",
        );
        drop(completion);
        assert_eq!(egress.in_use(), 0, "terminal egress releases it");
    }

    /// The aux shapes take no concurrency slot at all, so an aux flood
    /// cannot consume the capacity auctions contend for.
    #[test]
    fn the_aux_shapes_hold_no_auction_concurrency_slot() {
        let aux = AuxGate::new(1);
        let permit = aux.try_acquire().expect("capacity 1 admits");
        let (aux_lease, aux_completion) =
            admit(RouteAdmissionMarker::Aux, RouteEgressPermit::Aux(permit));
        assert!(
            aux_lease.auction_admission.is_none(),
            "`admit` is the aux shape and takes no auction-concurrency slot",
        );
        drop((aux_lease, aux_completion));

        let (pending_lease, pending_completion) = admit_pending(RouteAdmissionMarker::Aux);
        assert!(
            pending_lease.auction_admission.is_none(),
            "the bare pending shape holds no concurrency slot either",
        );
        drop((pending_lease, pending_completion));
    }

    /// A pending slot the handler never fills closes empty. That is the
    /// refusal / config-failure / timeout path, and it must be a no-op
    /// rather than a stranded resource.
    #[test]
    fn a_pending_slot_that_is_never_filled_closes_empty() {
        let (lease, completion) = admit_pending(RouteAdmissionMarker::Auction);
        drop(lease);
        // No panic, nothing to release: the whole assertion is that this
        // path is uneventful.
        drop(completion);
    }

    /// A handler reads the class back off the lease, which is the only
    /// thing the marker is for.
    #[test]
    fn the_lease_reports_which_gate_admitted_the_request() {
        let gate = EgressGate::new(1);
        let permit = gate.try_acquire().expect("capacity 1 admits");
        let (lease, completion) = admit(
            RouteAdmissionMarker::Auction,
            RouteEgressPermit::Auction(permit),
        );
        let AuctioneerIngressLease { admission, .. } = &lease;
        assert_eq!(*admission, RouteAdmissionMarker::Auction);
        drop(lease);
        drop(completion);
        assert_eq!(gate.in_use(), 0);
    }
}
