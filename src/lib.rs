use std::fs;
use anyhow::*;

mod datamine;
pub use datamine::*;

const DATAMINE_PATH: &str = "datamine.json";

pub fn read_datamine() -> Result<DataMine> {
    let data = fs::read(DATAMINE_PATH)
        .context("open datamine file")?;

    let datamine = serde_json::from_slice::<DataMine>(&data)
        .context("deserializing datamine")?;

    Ok(datamine)
}
