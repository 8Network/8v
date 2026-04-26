use lib_crate::Counter;
use std::collections::BTreeMap;

fn main() {
    let unused_x = 10;
    let mut c = Counter::new("test".to_string());
    c.increment();
    println!("count: {}", c.value());
    let sum = lib_crate::add(1, 2);
    println!("sum: {}", sum);
}
