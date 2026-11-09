// see: cans/15-operations.md#Multi-language-parity-gate
// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn split_camel(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if ch.is_uppercase() && index > 0 {
            let prev = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(|n| n.is_lowercase());
            if prev.is_lowercase() || prev.is_numeric() || (prev.is_uppercase() && next_lower) {
                parts.push(std::mem::take(&mut current));
            }
        }
        current.push(*ch);
    }
    parts.push(current);
    parts
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn screaming(name: &str) -> String {
    split_camel(name)
        .iter()
        .map(|p| p.to_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn kebab(name: &str) -> String {
    split_camel(name)
        .iter()
        .map(|p| p.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn camel(name: &str) -> String {
    if name.contains('_') {
        let mut out = String::new();
        for (index, part) in name.split('_').filter(|p| !p.is_empty()).enumerate() {
            if index == 0 {
                out.push_str(part);
            } else {
                let mut chars = part.chars();
                if let Some(first) = chars.next() {
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                }
            }
        }
        return out;
    }
    let parts = split_camel(name);
    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        let lower = part.to_lowercase();
        if index == 0 {
            out.push_str(&lower);
        } else {
            let mut chars = lower.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
    }
    out
}

// see: cans/10-attestation.md#Execution-receipts
pub fn snake(name: &str) -> String {
    split_camel(name)
        .iter()
        .map(|p| p.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn ts_sdk_name(rust_name: &str) -> String {
    if rust_name == "TelemetryExtension" {
        "ReceiptExtension".to_string()
    } else {
        rust_name.to_string()
    }
}

// see: cans/03-sdk-architecture.md#Python-SDK
pub fn py_sdk_name(rust_name: &str) -> String {
    if rust_name == "TelemetryExtension" {
        "ReceiptExtension".to_string()
    } else {
        rust_name.to_string()
    }
}

// see: cans/03-sdk-architecture.md#TypeScript-SDK
pub fn ts_native_record_name(rust_name: &str) -> String {
    if rust_name == "SnapshotDescriptor" {
        "NativeSnapshotRecord".to_string()
    } else {
        format!("Native{rust_name}Record")
    }
}
