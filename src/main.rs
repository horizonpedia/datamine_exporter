use std::collections::BTreeMap;
use anyhow::*;
use datamine_exporter::{ExtendedValue, get_cached_or_download_datamine};

const DATAMINE_PATH: &str = "datamine.json";

#[tokio::main]
async fn main() -> Result<()> {
    let vars = dotenv::vars()
        .collect::<BTreeMap<String, String>>();

    let api_key = vars.get("API_KEY")
        .context("API_KEY missing in .env")?;

    let datamine = get_cached_or_download_datamine(DATAMINE_PATH, api_key)
        .await
        .map_err(|e| anyhow!(e.to_string().replace(api_key, "<REDACTED>")))
        .context("failed to get datamine")?;

    println!();
    println!("##############");
    println!("### Sheets ###");
    println!("##############");
    println!();

    for sheet in &datamine.sheets {
        println!("{}", sheet.properties.title);
    }

    for sheet in &datamine.sheets {
        if sheet.properties.title != "Recipes" {
            continue;
        }

        println!();
        println!("######################");
        println!("### Recipe columns ###");
        println!("######################");
        println!();

        let grid = sheet.data.first().unwrap();
        let row1 = grid.row_data.first().unwrap();

        for cell in &row1.values {
            if let ExtendedValue::String { value } = cell.effective_value.as_ref().unwrap() {
                println!("{}", value);
            }
        }
    }

    Ok(())
}
