//! # Pretty Assertions (Sorted)
//!
//! This crate wraps the [pretty_assertions](https://raw.githubusercontent.com/colin-kiegel/rust-pretty-assertions) crate, which highlights differences
//! in a test failure via a colorful diff.
//!
//! However, the diff is based on the Debug output of the objects. For objects that
//! have non-deterministic output, eg. two HashMaps with close to the same contents, the diff
//! will be polluted and obscured with with false-positive differences like here:
//!
//! ![standard assertion](https://raw.githubusercontent.com/DarrenTsung/rust-pretty-assertions-sorted/fe860f070bdfb29a399a32ff9d3b98ca8d958326/images/non_deterministic.png)
//!
//! This is much easier to understand when the diff is sorted:
//!
//! ![sorted assertion](https://raw.githubusercontent.com/DarrenTsung/rust-pretty-assertions-sorted/fe860f070bdfb29a399a32ff9d3b98ca8d958326/images/sorted.png)
//!
//! This is a pretty trivial example, you could solve this instead by converting the HashMap to
//! a BTreeMap in your tests. But it's not always feasible to replace the types with ordered
//! versions, especially for HashMaps that are deeply nested in types outside of your control.
//!
//! To use the sorted version, import like this:
//!
//! ```rust
//! use pretty_assertions_sorted::{assert_eq, assert_eq_sorted};
//! ```
//!
//! `assert_eq` is provided as a re-export of `pretty_assertions::assert_eq` and should
//! be used if you don't want the Debug output to be sorted, or if the Debug output can't
//! be sorted (not supported types, eg. f64::NEG_INFINITY, or custom Debug output).
//!
//! ## Tip
//!
//! Specify it as [`[dev-dependencies]`](http://doc.crates.io/specifying-dependencies.html#development-dependencies)
//! and it will only be used for compiling tests, examples, and benchmarks.
//! This way the compile time of `cargo build` won't be affected!
use std::fmt;

use darrentsung_debug_parser::*;
pub use pretty_assertions::{assert_eq, assert_ne, Comparison};

/// This is a wrapper with similar functionality to [`assert_eq`], however, the
/// [`Debug`] representation is sorted to provide deterministic output.
///
/// Not all [`Debug`] representations are sortable yet and this doesn't work with
/// custom [`Debug`] implementations that don't conform to the format that #[derive(Debug)]
/// uses, eg. `fmt.debug_struct()`, `fmt.debug_map()`, etc.
///
/// Don't use this if you want to test the ordering of the types that are sorted, since
/// sorting will clobber any previous ordering.
///
/// Potential use-cases that aren't implemented yet:
/// * Blocklist for field names that shouldn't be sorted
/// * Sorting more than just maps (struct fields, lists, etc.)
#[macro_export]
macro_rules! assert_eq_sorted {
    ($left:expr, $right:expr$(,)?) => ({
        $crate::assert_eq_sorted!(@ $left, $right, "", "");
    });
    ($left:expr, $right:expr, $($arg:tt)*) => ({
        $crate::assert_eq_sorted!(@ $left, $right, ": ", $($arg)+);
    });
    (@ $left:expr, $right:expr, $maybe_semicolon:expr, $($arg:tt)*) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                let left_val = $crate::SortedDebug::new(left_val);
                let right_val = $crate::SortedDebug::new(right_val);
                
                if !(format!("{:?}", left_val) == format!("{:?}", right_val)) {
                    // We create the comparison string outside the panic! call
                    // because creating the comparison string could panic itself.
                    let comparison_string = $crate::Comparison::new(
                        &left_val,
                        &right_val
                    ).to_string();
                    ::core::panic!("assertion failed: `(left == right)`{}{}\
                       \n\
                       \n{}\
                       \n",
                       $maybe_semicolon,
                       format_args!($($arg)*),
                       comparison_string,
                    )
                }
            }
        }
    });
}

/// New-type wrapper around an object that sorts the fmt::Debug output when displayed for
/// deterministic output.
///
/// This works through parsing the output and sorting the `debug_map()` type.
///
/// DISCLAIMER: This Debug implementation will panic if the inner value's Debug
/// representation can't be sorted. This is used to notify users when used in tests. An
/// alternative solution of falling back to non-sorted could be implemented.
///
/// Potential use-cases that aren't implemented yet:
/// * Blocklist for field names that shouldn't be sorted
/// * Sorting more than just maps (struct fields, lists, etc.)
pub struct SortedDebug<T>(T);

impl<T> SortedDebug<T> {
    pub fn new(v: T) -> Self {
        Self(v)
    }
}

impl<T: fmt::Debug> fmt::Debug for SortedDebug<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut value = match parse(&format!("{:?}", self.0)) {
            Ok(value) => value,
            Err(err) => {
                ::core::panic!("Failed to parse Debug output for sorting (please use `assert_eq!` instead and/or file an issue for your use-case)!\nError: {}", err)
            }
        };

        sort_maps(&mut value);

        // Replace one-line non-exhaustive objects with empty brackets separated by
        // newlines. This changes output like: "Foo { .. }" with "Foo {\n}". "Foo {\n}" is
        // more desirable because it diffs better against some multi-line output of "Foo {
        // value: 10.0 }" (imagine the newlines please).
        let formatted_output = format!("{:#?}", value).replace("{ .. }", "{\n}");
        fmt::Display::fmt(&formatted_output, f)
    }
}

fn sort_maps(v: &mut Value) {
    match v {
        Value::Struct(s) => {
            for ident_value_or_non_exhaustive in &mut s.values {
                match ident_value_or_non_exhaustive {
                    OrNonExhaustive::Value(ident_value) => {
                        sort_maps(&mut ident_value.value);
                    }
                    OrNonExhaustive::NonExhaustive => (),
                }
            }
        }
        Value::Set(s) => {
            s.values.sort_by(|a, b| a.cmp(&b));
            for child_v in &mut s.values {
                sort_maps(child_v);
            }
        }
        Value::Map(map) => {
            map.values.sort_by(|a, b| a.key.cmp(&b.key));

            for key_value in &mut map.values {
                sort_maps(&mut key_value.key);
                sort_maps(&mut key_value.value);
            }
        }
        Value::List(l) => {
            l.values.sort_by(|a, b| a.cmp(&b));
            for child_v in &mut l.values {
                sort_maps(child_v);
            }
        }
        Value::Tuple(t) => {
            t.values.sort_by(|a, b| a.cmp(&b));
            for child_v in &mut t.values {
                sort_maps(child_v);
            }
        }
        // No need to recurse for Term variant.
        Value::Term(_) => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::assert_eq;
    use std::collections::HashMap;

    const TEST_RERUNS_FOR_DETERMINISM: u32 = 100;

    fn sorted_debug<T: fmt::Debug>(v: T) -> String {
        format!("{:#?}", SortedDebug(v))
    }

    #[test]
    fn noop_sorts() {
        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            assert_eq!(sorted_debug(2), "2");
        }
    }

    #[test]
    fn sorts_hashmap() {
        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            // Note that we have to create the HashMaps each try
            // in order to induce non-determinism in the debug output.
            let item = {
                let mut map = HashMap::new();
                map.insert(1, true);
                map.insert(2, true);
                map.insert(20, true);
                map
            };

            let expected = indoc!(
                "{
                    1: true,
                    2: true,
                    20: true,
                }"
            );
            assert_eq!(sorted_debug(item), expected);
        }
    }

    #[test]
    fn sorts_hashmaps_with_non_exhaustive_object_values() {
        #[allow(unused)]
        struct Foo {
            value: f32,
        }

        impl fmt::Debug for Foo {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("Foo")
                    .field("value", &self.value)
                    .finish_non_exhaustive()
            }
        }

        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = {
                let mut map = HashMap::new();
                map.insert(1, Foo { value: 10.1 });
                map.insert(32, Foo { value: 2.0 });
                map.insert(2, Foo { value: -1.5 });
                map
            };

            let expected = indoc!(
                "{
                    1: Foo {
                        value: 10.1,
                        ..
                    },
                    2: Foo {
                        value: -1.5,
                        ..
                    },
                    32: Foo {
                        value: 2.0,
                        ..
                    },
                }"
            );
            assert_eq!(sorted_debug(item), expected);
        }
    }

    #[test]
    fn sorts_list() {
        #[allow(unused)]
        #[derive(Debug)]
        struct Foo {
            value: f32,
        }

        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = {
                let mut map = Vec::new();
                map.insert(0, Foo { value: 10.1 });
                map.insert(1, Foo { value: 2.0 });
                map.insert(2, Foo { value: -1.5 });
                map
            };

            let expected = indoc!(
                "[
                    Foo {
                        value: -1.5,
                    },
                    Foo {
                        value: 10.1,
                    },
                    Foo {
                        value: 2.0,
                    },
                ]"
            );
            let sorted = sorted_debug(item);
            println!("{}", sorted);
            // let comparison = Comparison::new(
            //     expected,
            //     sorted.as_str()
            // ).to_string();
            println!("{}", expected);
            assert_eq!(sorted, expected);
        }
    }

    #[test]
    fn test_list_assert_eq_sorted() {
        #[allow(unused)]
        #[derive(Debug, PartialEq)]
        struct Foo {
            value: f32,
        }

        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = {
                let mut map = Vec::new();
                map.insert(0, Foo { value: 10.1 });
                map.insert(1, Foo { value: 2.0 });
                map.insert(2, Foo { value: -1.5 });
                map
            };

            let expected = vec![
                Foo {
                    value: -1.5,
                },
                Foo {
                    value: 10.1,
                },
                Foo {
                    value: 2.0,
                },
            ];
            assert_eq_sorted!(item, expected);
        }
    }

    #[test]
    fn test_list_assert_eq_sorted_1() {
        #[allow(unused)]
        #[derive(Debug, Clone, PartialEq)]
        struct Foo {
            value: f32,
        }


        let item = {
            let mut map = Vec::new();
            map.insert(0, Foo { value: 10.1 });
            map.insert(1, Foo { value: 2.0 });
            map.insert(2, Foo { value: -1.5 });
            map
        };

        let expected = vec![
            Foo {
                value: -1.5,
            },
            Foo {
                value: 10.1,
            },
            Foo {
                value: 2.0,
            },
        ];
        
        assert_eq_sorted!(item, expected);
        
    }

    #[test]
    fn sorts_object_with_hashmap() {
        #[derive(Debug)]
        #[allow(unused)]
        struct Foo {
            bar: Bar,
        }

        #[derive(Debug)]
        #[allow(unused)]
        struct Bar {
            count: HashMap<&'static str, Zed>,
            value: usize,
        }

        #[derive(Debug)]
        struct Zed;

        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = Foo {
                bar: Bar {
                    count: {
                        let mut map = HashMap::new();
                        map.insert("hello world", Zed);
                        map.insert("lorem ipsum", Zed);
                        map
                    },
                    value: 200,
                },
            };

            let expected = indoc!(
                "Foo {
                    bar: Bar {
                        count: {
                            \"hello world\": Zed,
                            \"lorem ipsum\": Zed,
                        },
                        value: 200,
                    },
                }"
            );
            assert_eq!(sorted_debug(item), expected);
        }
    }

    #[test]
    fn hashmap_with_object_values() {
        #[derive(Debug)]
        #[allow(unused)]
        struct Foo {
            value: f32,
            bar: Vec<Bar>,
        }

        #[derive(Debug)]
        #[allow(unused)]
        struct Bar {
            elo: i32,
        }

        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = {
                let mut map = HashMap::new();
                map.insert(
                    "foo",
                    Foo {
                        value: 12.2,
                        bar: vec![Bar { elo: 200 }, Bar { elo: -12 }],
                    },
                );
                map.insert(
                    "foo2",
                    Foo {
                        value: -0.2,
                        bar: vec![],
                    },
                );
                map
            };

            let expected = indoc!(
                "{
                    \"foo\": Foo {
                        value: 12.2,
                        bar: [
                            Bar {
                                elo: -12,
                            },
                            Bar {
                                elo: 200,
                            },
                        ],
                    },
                    \"foo2\": Foo {
                        value: -0.2,
                        bar: [],
                    },
                }"
            );
            assert_eq!(sorted_debug(item), expected);
        }
    }

    #[test]
    fn hashmap_with_object_keys() {
        #[derive(Debug, PartialEq, Eq, Hash)]
        struct Foo {
            value: i32,
            bar: Vec<Bar>,
        }

        #[derive(Debug, PartialEq, Eq, Hash)]
        struct Bar {
            elo: i32,
        }

        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = {
                let mut map = HashMap::new();
                map.insert(
                    Foo {
                        value: 12,
                        bar: vec![Bar { elo: 200 }, Bar { elo: -12 }],
                    },
                    "foo",
                );
                map.insert(
                    Foo {
                        value: -2,
                        bar: vec![],
                    },
                    "foo2",
                );
                map
            };

            let expected = indoc!(
                "{
                    Foo {
                        value: -2,
                        bar: [],
                    }: \"foo2\",
                    Foo {
                        value: 12,
                        bar: [
                            Bar {
                                elo: -12,
                            },
                            Bar {
                                elo: 200,
                            },
                        ],
                    }: \"foo\",
                }"
            );
            let sorted = sorted_debug(item);
            // println!("{}", sorted);
            assert_eq!(sorted, expected);
        }
    }

    #[test]
    fn hashmap_with_chrono_naivedate() {
        for _ in 0..TEST_RERUNS_FOR_DETERMINISM {
            let item = {
                let mut map = HashMap::new();
                map.insert(chrono::NaiveDate::from_ymd_opt(2000, 2, 14).unwrap(), "foo");
                map.insert(chrono::NaiveDate::from_ymd_opt(2001, 4, 2).unwrap(), "foo");
                map
            };

            dbg!(&item);

            let expected = indoc!(
                "{
                    2000-02-14: \"foo\",
                    2001-04-02: \"foo\",
                }"
            );
            assert_eq!(sorted_debug(item), expected);
        }
    }

    #[test]
    #[should_panic(
        expected = "Failed to parse Debug output for sorting (please use `assert_eq!` instead and/or file an issue for your use-case)!
Error: Failed to consume all of string!
Value:
Object

Rest:
\" {\\\"a\\\": Number(0)}\""
    )]
    fn panics_when_expression_cant_be_sorted() {
        assert_eq_sorted!(serde_json::json!({"a":0}), "2");
    }

    #[derive(PartialEq)]
    #[allow(unused)]
    struct FooWithOptionalField {
        value: Option<f32>,
    }

    impl fmt::Debug for FooWithOptionalField {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut foo = f.debug_struct("FooWithOptionalField");
            if let Some(value) = &self.value {
                foo.field("value", value);
                foo.finish()
            } else {
                foo.finish_non_exhaustive()
            }
        }
    }

    /// Test that the value field is displayed as missing (colored red) for optional fields
    /// on non-exhaustive Debug implementations.
    #[test]
    #[should_panic(expected = "FooWithOptionalField {\n\u{1b}[31m<    value: 2.0,\u{1b}[0m\n }")]
    fn ui_looks_right_for_non_exhaustive_optional_fields() {
        assert_eq_sorted!(
            FooWithOptionalField { value: Some(2.0) },
            FooWithOptionalField { value: None }
        );
    }

    #[test]
    fn outputs_nice_output_for_non_exhaustive_objects_with_optional_fields() {
        assert_eq!(
            sorted_debug(FooWithOptionalField { value: Some(10.0) }),
            indoc!(
                "FooWithOptionalField {
                    value: 10.0,
                }"
            )
        );

        assert_eq!(
            sorted_debug(FooWithOptionalField { value: None }),
            indoc!(
                "FooWithOptionalField {
                }"
            )
        );
    }
}
