//! Export runtime-generated forms for inspection; does not write a database.
use latin_vocab::{Word, morphology::Verb};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seed: serde_json::Value = serde_json::from_str(include_str!("../data/swadesh.json"))?;
    let mut output = serde_json::Map::new();
    for value in seed["words"].as_array().unwrap() {
        let w: Word = serde_json::from_value(value.clone())?;
        w.validate()?;
        if let Some(Verb::Regular(v)) = w.verb {
            output.insert(w.number.to_string(), serde_json::to_value(v.generate()?)?);
        }
    }
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
