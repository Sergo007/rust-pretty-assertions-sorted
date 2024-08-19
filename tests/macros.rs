#[allow(clippy::eq_op)]
mod assert_eq {
    #[test]
    fn passes() {
        let a = "some value";
        ::pretty_assertions_sorted_fork::assert_eq_all_sorted!(a, a);
    }
}
