// see: cans/15-operations.md#Multi-language-parity-gate
use std::fs;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let out = args.first().map(String::as_str).unwrap_or("vectors.json");
    let names = kelir_plugin_testkit::fixtures::canonical_vector_names();
    let mut text = String::from("{\"vectors\":[");
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str("{\"name\":\"");
        text.push_str(name);
        text.push_str("\"}");
    }
    text.push_str("]}");
    fs::write(out, text)?;
    println!("vectors: wrote {} vector(s) to {out}", names.len());
    Ok(())
}
