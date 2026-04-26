fn main() {
    let items = vec![10, 20, 30, 40, 50];
    let result = sum_range(&items, 1, 3);
    println!("Sum of items[1..3]: {result}");
}

/// Sum elements from index `start` to `end` (inclusive).
fn sum_range(items: &[i32], start: usize, end: usize) -> i32 {
    let mut total = 0;
    for i in start..end {
        total += items[i];
    }
    total
}

fn format_report(items: &[i32]) -> String {
    let mut report = String::new();
    report.push_str("=== Report ===\n");
    for (i, item) in items.iter().enumerate() {
        report.push_str(&format!("  [{i}] = {item}\n"));
    }
    report.push_str(&format!("  Total: {}\n", items.iter().sum::<i32>()));
    report.push_str(&format!("  Count: {}\n", items.len()));
    report.push_str(&format!(
        "  Average: {}\n",
        items.iter().sum::<i32>() / items.len() as i32,
    ));
    report.push_str("=== End ===\n");
    report
}

fn validate_range(start: usize, end: usize, len: usize) -> Result<(), String> {
    if start >= len {
        return Err(format!("start {start} out of bounds (len={len})"));
    }
    if end >= len {
        return Err(format!("end {end} out of bounds (len={len})"));
    }
    if start > end {
        return Err(format!("start {start} > end {end}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_range_inclusive() {
        let items = vec![10, 20, 30, 40, 50];
        assert_eq!(sum_range(&items, 1, 3), 90); // This test FAILS due to the bug
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range(0, 4, 5).is_ok());
        assert!(validate_range(5, 4, 5).is_err());
    }
}
