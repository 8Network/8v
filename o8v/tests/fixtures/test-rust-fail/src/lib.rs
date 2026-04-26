pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_passes() {
        assert_eq!(add(1, 2), 3);
    }

    #[test]
    fn it_fails_intentionally() {
        assert_eq!(add(1, 2), 99, "intentional failure");
    }
}
