use std::{fs, collections::BTreeMap};
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

    let recipes = datamine.find_sheet_by_title("Recipes")
        .context("Failed to find recipes sheet")?;

    let recipes = recipes.json_rows()
        .context("Failed to get recipes json rows")?;

    let recipes_json = serde_json::to_vec_pretty(&recipes)
        .context("Failed to serialize recipes")?;

    fs::create_dir_all("export")
        .context("Failed to create export directory")?;

    fs::write("export/recipes.json", &recipes_json)
        .context("Failed to write recipes.json")?;

    Ok(())
}
