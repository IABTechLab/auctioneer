//! Executes explicit generated matching with observed constructors and cache owners.

#![cfg(not(target_arch = "wasm32"))]

extern crate regex_automata as engine_automata;
extern crate self as regex_automata;

pub use engine_automata::{HalfMatch, Input, MatchKind, util};

#[cfg(test)]
pub mod meta {
    use engine_automata::meta::{Builder as EngineBuilder, Regex as EngineRegex};
    pub use engine_automata::meta::{Cache, Config};
    use engine_automata::util::syntax;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    pub static BUILDS: AtomicUsize = AtomicUsize::new(0);
    pub static CACHES: AtomicUsize = AtomicUsize::new(0);
    pub static SEARCHES: AtomicUsize = AtomicUsize::new(0);
    pub static FAIL_NEXT: AtomicBool = AtomicBool::new(false);
    pub static DEFAULT_LIMITS: AtomicBool = AtomicBool::new(false);
    pub static ASYMMETRIC_LIMITS: AtomicBool = AtomicBool::new(false);

    #[derive(Debug)]
    pub struct ConstructorFailure;

    pub struct Regex {
        inner: EngineRegex,
    }

    impl Regex {
        #[must_use]
        pub fn config() -> Config {
            EngineRegex::config()
        }

        #[must_use]
        pub fn builder() -> Builder {
            Builder {
                inner: EngineRegex::builder(),
                accepted: false,
            }
        }

        #[must_use]
        pub fn create_cache(&self) -> Cache {
            CACHES.fetch_add(1, Ordering::SeqCst);
            self.inner.create_cache()
        }

        #[must_use]
        /// # Panics
        /// Panics if generated matching stops requesting earliest matches.
        pub fn search_half_with(
            &self,
            cache: &mut Cache,
            input: &super::Input<'_>,
        ) -> Option<super::HalfMatch> {
            assert!(input.get_earliest());
            SEARCHES.fetch_add(1, Ordering::SeqCst);
            self.inner.search_half_with(cache, input)
        }
    }

    // Deliberately no Clone or pooled matching methods: using either in
    // generated code must fail compilation, not merely change a counter.
    pub struct Builder {
        inner: EngineBuilder,
        accepted: bool,
    }

    impl Builder {
        pub fn configure(&mut self, config: Config) -> &mut Self {
            let (nfa, hybrid) = if DEFAULT_LIMITS.load(Ordering::SeqCst) {
                (10_usize << 20_u32, 2_usize << 20_u32)
            } else if ASYMMETRIC_LIMITS.load(Ordering::SeqCst) {
                (98_304, 0x0002_0000)
            } else {
                (0x0001_0000, 0x0001_0000)
            };
            self.accepted = config.get_match_kind() == super::MatchKind::LeftmostFirst
                && config.get_utf8_empty()
                && config.get_nfa_size_limit() == Some(nfa)
                && config.get_hybrid_cache_capacity() == hybrid
                && config.get_pool_capacity() == 1;
            self.inner.configure(config);
            self
        }

        pub fn syntax(&mut self, config: syntax::Config) -> &mut Self {
            self.accepted &= config.get_utf8();
            self.inner.syntax(config);
            self
        }

        /// # Errors
        /// Refuses changed limits/defaults, injected failure or an engine error.
        pub fn build(&self, pattern: &str) -> Result<Regex, ConstructorFailure> {
            if !self.accepted {
                return Err(ConstructorFailure);
            }
            BUILDS.fetch_add(1, Ordering::SeqCst);
            if FAIL_NEXT.swap(false, Ordering::SeqCst) {
                return Err(ConstructorFailure);
            }
            self.inner
                .build(pattern)
                .map(|inner| Regex { inner })
                .map_err(|_error| ConstructorFailure)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::meta::{ASYMMETRIC_LIMITS, BUILDS, CACHES, DEFAULT_LIMITS, FAIL_NEXT, SEARCHES};
    use serde_json::json;
    use std::sync::Barrier;
    use std::sync::atomic::Ordering;
    use std::thread;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Failed;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Good;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, cache = explicit },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Defaults;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, cache = explicit, size_limit = 98_304, dfa_size_limit = 131_072 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Asymmetric;

    #[test]
    fn explicit_factories_force_all_constructors_cache_failures_and_own_each_search_cache() {
        assert_failed_factories_never_create_caches();

        assert!(Good::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 4);
        assert_eq!(CACHES.load(Ordering::SeqCst), 0);
        let mut first = Good::new_search_cache().unwrap();
        let mut second = Good::new_search_cache().unwrap();
        assert_eq!(CACHES.load(Ordering::SeqCst), 4);
        let input = json!({"letters": "abc", "digits": "123"});
        assert!(Good::is_valid(&input, &mut first));
        assert!(Good::is_valid(&input, &mut second));
        assert_eq!(SEARCHES.load(Ordering::SeqCst), 4);
        assert_eq!(BUILDS.load(Ordering::SeqCst), 4);
        assert_eq!(CACHES.load(Ordering::SeqCst), 4);

        let barrier = Barrier::new(4);
        thread::scope(|scope| {
            for _ in 0_usize..4 {
                scope.spawn(|| {
                    let mut cache = Good::new_search_cache().unwrap();
                    barrier.wait();
                    assert!(Good::is_valid(&input, &mut cache));
                });
            }
        });
        assert_eq!(BUILDS.load(Ordering::SeqCst), 4);
        assert_eq!(CACHES.load(Ordering::SeqCst), 12);
        assert_eq!(SEARCHES.load(Ordering::SeqCst), 12);

        DEFAULT_LIMITS.store(true, Ordering::SeqCst);
        let mut defaults = Defaults::new_search_cache().unwrap();
        assert!(Defaults::is_valid(&input, &mut defaults));
        DEFAULT_LIMITS.store(false, Ordering::SeqCst);
        assert_eq!(BUILDS.load(Ordering::SeqCst), 6);
        assert_eq!(CACHES.load(Ordering::SeqCst), 14);
        assert_eq!(SEARCHES.load(Ordering::SeqCst), 14);

        ASYMMETRIC_LIMITS.store(true, Ordering::SeqCst);
        let mut asymmetric = Asymmetric::new_search_cache().unwrap();
        assert!(Asymmetric::is_valid(&input, &mut asymmetric));
        ASYMMETRIC_LIMITS.store(false, Ordering::SeqCst);
        assert_eq!(BUILDS.load(Ordering::SeqCst), 8);
        assert_eq!(CACHES.load(Ordering::SeqCst), 16);
        assert_eq!(SEARCHES.load(Ordering::SeqCst), 16);
    }

    fn assert_failed_factories_never_create_caches() {
        FAIL_NEXT.store(true, Ordering::SeqCst);
        assert!(Failed::new_search_cache().is_none());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 2);
        assert!(!FAIL_NEXT.load(Ordering::SeqCst));
        assert!(!Failed::initialize_regexes());
        assert!(Failed::new_search_cache().is_none());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 2);
        assert_eq!(CACHES.load(Ordering::SeqCst), 0);
        assert_eq!(SEARCHES.load(Ordering::SeqCst), 0);
    }
}
