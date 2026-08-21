use serde::Serialize;

pub(crate) fn print_json<T: Serialize>(value: &T, pretty: bool) {
    if pretty {
        if let Ok(serialized) = serde_json::to_string_pretty(value) {
            println!("{serialized}");
            return;
        }
    }
    println!("{}", serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()));
}
