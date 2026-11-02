//! Executes generated code with observed runtime constructors, not a second compiler.

#![cfg(not(target_arch = "wasm32"))]

extern crate jsonschema as engine;
extern crate self as jsonschema;

pub use engine::*;

// The macro still uses the real engine to compile the schema. Its generated
// runtime calls resolve here, allowing a constructor failure without changing
// a schema, production engine, or compiler testing hook.
#[cfg(test)]
pub mod __private {
    pub use engine::__private::*;

    pub mod regex {
        pub use engine::__private::regex::Regex;
        use engine::__private::regex::RegexBuilder as RuntimeRegexBuilder;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        pub static BUILDS: AtomicUsize = AtomicUsize::new(0);
        pub static FAIL_NEXT: AtomicBool = AtomicBool::new(false);

        #[derive(Debug)]
        pub struct ConstructorFailure;

        pub struct RegexBuilder {
            inner: RuntimeRegexBuilder,
            limits: [Option<usize>; 2],
        }

        impl RegexBuilder {
            #[must_use]
            pub fn new(pattern: &str) -> Self {
                Self {
                    inner: RuntimeRegexBuilder::new(pattern),
                    limits: [None; 2],
                }
            }

            pub fn size_limit(&mut self, limit: usize) -> &mut Self {
                self.limits[0] = Some(limit);
                self.inner.size_limit(limit);
                self
            }

            pub fn dfa_size_limit(&mut self, limit: usize) -> &mut Self {
                self.limits[1] = Some(limit);
                self.inner.dfa_size_limit(limit);
                self
            }

            /// # Errors
            /// Refuses changed/missing limits, injected failure, or an engine error.
            pub fn build(&self) -> Result<Regex, ConstructorFailure> {
                if self.limits != [Some(0x0001_0000); 2] {
                    return Err(ConstructorFailure);
                }
                BUILDS.fetch_add(1, Ordering::SeqCst);
                if FAIL_NEXT.swap(false, Ordering::SeqCst) {
                    return Err(ConstructorFailure);
                }
                self.inner.build().map_err(|_error| ConstructorFailure)
            }
        }
    }

    pub mod fancy_regex {
        pub use engine::__private::fancy_regex::Regex;
        use engine::__private::fancy_regex::RegexBuilder as RuntimeRegexBuilder;
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        pub static BUILDS: AtomicUsize = AtomicUsize::new(0);
        pub static FAIL_NEXT: AtomicBool = AtomicBool::new(false);

        #[derive(Debug)]
        pub struct ConstructorFailure;

        pub struct RegexBuilder {
            inner: RuntimeRegexBuilder,
            limits: [Option<usize>; 3],
        }

        impl RegexBuilder {
            #[must_use]
            pub fn new(pattern: &str) -> Self {
                Self {
                    inner: RuntimeRegexBuilder::new(pattern),
                    limits: [None; 3],
                }
            }

            pub fn backtrack_limit(&mut self, limit: usize) -> &mut Self {
                self.limits[0] = Some(limit);
                self.inner.backtrack_limit(limit);
                self
            }

            pub fn delegate_size_limit(&mut self, limit: usize) -> &mut Self {
                self.limits[1] = Some(limit);
                self.inner.delegate_size_limit(limit);
                self
            }

            pub fn delegate_dfa_size_limit(&mut self, limit: usize) -> &mut Self {
                self.limits[2] = Some(limit);
                self.inner.delegate_dfa_size_limit(limit);
                self
            }

            /// # Errors
            /// Refuses changed/missing limits, injected failure, or an engine error.
            pub fn build(&self) -> Result<Regex, ConstructorFailure> {
                if self.limits != [Some(1_000), Some(0x0001_0000), Some(0x0001_0000)] {
                    return Err(ConstructorFailure);
                }
                BUILDS.fetch_add(1, Ordering::SeqCst);
                if FAIL_NEXT.swap(false, Ordering::SeqCst) {
                    return Err(ConstructorFailure);
                }
                self.inner.build().map_err(|_error| ConstructorFailure)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::__private::regex::{BUILDS, FAIL_NEXT};
    use serde_json::{Value, json};
    use std::sync::Barrier;
    use std::sync::atomic::Ordering;
    use std::thread;

    #[engine::validator(path = "tests/support/fake_bidder/schemas/compiler-integer-keywords.json", draft = Draft4)]
    struct NoPatterns;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 }
    )]
    struct Patterns;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Concurrent;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 }
    )]
    struct ValidationFirst;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 }
    )]
    struct Mixed;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 }
    )]
    struct Failed;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = fancy_regex, backtrack_limit = 1_000, size_limit = 65_536, dfa_size_limit = 65_536 }
    )]
    struct FancyPatterns;

    #[engine::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = fancy_regex, backtrack_limit = 1_000, size_limit = 65_536, dfa_size_limit = 65_536 }
    )]
    struct FancyFailed;

    fn assert_mixed_access(instance: &Value) {
        let barrier = &Barrier::new(8);
        thread::scope(|scope| {
            for index in 0_usize..8 {
                scope.spawn(move || {
                    barrier.wait();
                    if index.is_multiple_of(2) {
                        assert!(Mixed::initialize_regexes());
                    } else {
                        assert!(Mixed::is_valid(instance));
                    }
                });
            }
        });
    }

    fn assert_failure_is_shared_without_retries(instance: &Value) {
        FAIL_NEXT.store(true, Ordering::SeqCst);
        let barrier = Barrier::new(8);
        thread::scope(|scope| {
            for _ in 0_usize..8 {
                scope.spawn(|| {
                    barrier.wait();
                    assert!(!Failed::initialize_regexes());
                });
            }
        });
        assert_eq!(BUILDS.load(Ordering::SeqCst), 10);
        assert!(!FAIL_NEXT.load(Ordering::SeqCst));
        assert!(!Failed::initialize_regexes());
        assert!(!Failed::is_valid(instance));
        assert!(Failed::validate(instance).is_err());
        assert!(Failed::iter_errors(instance).count() > 0);
        assert_eq!(BUILDS.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn generated_initializers_force_every_constructor_once_and_cache_failures() {
        assert!(NoPatterns::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 0);
        assert!(!Patterns::is_valid(&json!({})));
        assert_eq!(BUILDS.load(Ordering::SeqCst), 0);
        assert!(Patterns::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 2);
        let valid = json!({"letters": "abc", "digits": "123"});
        let invalid = json!({"letters": "A", "digits": "x"});
        assert!(Patterns::is_valid(&valid));
        Patterns::validate(&valid).unwrap();
        assert_eq!(Patterns::iter_errors(&valid).count(), 0);
        assert!(!Patterns::is_valid(&invalid));
        assert!(Patterns::validate(&invalid).is_err());
        assert!(Patterns::iter_errors(&invalid).count() > 0);
        assert!(Patterns::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 2);

        let barrier = Barrier::new(8);
        thread::scope(|scope| {
            for _ in 0_usize..8 {
                scope.spawn(|| {
                    barrier.wait();
                    assert!(Concurrent::initialize_regexes());
                });
            }
        });
        assert!(Concurrent::is_valid(&valid));
        assert_eq!(BUILDS.load(Ordering::SeqCst), 4);

        assert!(ValidationFirst::is_valid(&valid));
        assert_eq!(BUILDS.load(Ordering::SeqCst), 6);
        assert!(ValidationFirst::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 6);

        assert_mixed_access(&valid);
        assert_eq!(BUILDS.load(Ordering::SeqCst), 8);

        assert_failure_is_shared_without_retries(&valid);
    }

    #[test]
    fn fancy_initializers_preserve_limits_and_cache_failures_without_short_circuit() {
        use super::__private::fancy_regex::{BUILDS, FAIL_NEXT};
        assert!(!FancyPatterns::is_valid(&json!({})));
        assert_eq!(BUILDS.load(Ordering::SeqCst), 0);
        assert!(FancyPatterns::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 2);
        let valid = json!({"letters": "abc", "digits": "123"});
        assert!(FancyPatterns::is_valid(&valid));
        FancyPatterns::validate(&valid).unwrap();
        assert_eq!(FancyPatterns::iter_errors(&valid).count(), 0);
        assert!(!FancyPatterns::is_valid(
            &json!({"letters": "A", "digits": "x"})
        ));
        assert!(FancyPatterns::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 2);
        FAIL_NEXT.store(true, Ordering::SeqCst);
        assert!(!FancyFailed::initialize_regexes());
        assert_eq!(BUILDS.load(Ordering::SeqCst), 4);
        assert!(!FancyFailed::initialize_regexes());
        assert!(!FancyFailed::is_valid(&valid));
        assert!(FancyFailed::validate(&valid).is_err());
        assert!(FancyFailed::iter_errors(&valid).count() > 0);
        assert_eq!(BUILDS.load(Ordering::SeqCst), 4);
    }
}
