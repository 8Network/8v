fn format_items(items: &Vec<i32>) -> String {
    let mut out = String::new();
    for i in 0..items.len() {
        let item = items[i].clone();
        out.push_str(&format!("{}\n", item));
    }
    return out;
}

fn main() {
    let items = vec![1, 2, 3, 4, 5];
    let s = format_items(&items);
    println!("{}", s);
}
