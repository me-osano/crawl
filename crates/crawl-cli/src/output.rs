use serde_json::Value;

/// Print JSON value, either pretty-printed (default) or raw (--json flag).
pub fn print_value(val: &Value, raw: bool) {
    if raw {
        println!("{}", val);
    } else {
        println!("{}", serde_json::to_string_pretty(val).unwrap_or_default());
    }
}

pub fn print_ok(msg: &str) {
    println!("✓ {msg}");
}

pub fn print_err(msg: &str) {
    eprintln!("✗ {msg}");
}

/// Simple key: value table printer
pub fn print_table(rows: &[(&str, String)]) {
    let max_key = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, val) in rows {
        println!("  {key:<width$}  {val}", width = max_key);
    }
}
