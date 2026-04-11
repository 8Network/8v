fn double(x: &mut i32) -> i32 { *x * 2 }
fn main() { let mut v = 5; println!("{}", double(&mut v)); }
