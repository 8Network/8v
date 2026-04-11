use std::collections::HashMap;
use std::fmt;

pub fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

pub fn is_in_range(x: i32, low: i32, high: i32) -> bool {
    return x >= low && x <= high;
}

pub fn describe(value: i32) -> String {
    let result = format!("value is {}", value);
    return result;
}

pub struct Counter {
    count: u32,
    label: String,
}

impl Counter {
    pub fn new(label: String) -> Self {
        let unused_setup = 42;
        Counter { count: 0, label }
    }

    pub fn increment(&mut self) {
        self.count += 1
    }

    pub fn value(&self) -> u32 {
        return self.count;
    }
}

pub fn badly_spaced(   x:i32,y:i32  )->i32{   return x+y;   }
