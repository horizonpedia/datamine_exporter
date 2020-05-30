use std::collections::BTreeMap;
use anyhow::*;
use datamine_exporter::get_cached_or_download_datamine;

const DATAMINE_PATH: &str = "datamine.json";

#[tokio::main]
async fn main() -> Result<()> {
    let vars = dotenv::vars()
        .collect::<BTreeMap<String, String>>();

    let api_key = vars.get("API_KEY")
        .context("API_KEY missing in .env")?;

    if let Err(err) = run(api_key).await {
        let err = format!("{:?}", err).replace(api_key, "<REDACTED>");
        println!("{}", err);
    }

    Ok(())
}

async fn run(api_key: &str) -> Result<()> {
    let datamine = get_cached_or_download_datamine(DATAMINE_PATH, api_key)
        .await
        .context("failed to get datamine")?;

    println!();
    println!("##############");
    println!("### Sheets ###");
    println!("##############");
    println!();

    for sheet in &datamine.sheets {
        println!("{}", sheet.properties.title);
    }

    let recipes = datamine.find_sheet_by_title("Recipes")
        .context("Failed to find recipes sheet")?;

    println!();
    println!("######################");
    println!("### Recipe columns ###");
    println!("######################");
    println!();

    let titles = recipes.column_titles()
        .context("Failed to get recipe column titles")?;

    for title in titles {
        println!("{}", title);
    }

    Ok(())
}
