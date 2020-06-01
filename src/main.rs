use std::{fs, collections::BTreeMap};
use anyhow::*;
use datamine_exporter::{Sheet, get_cached_or_download_datamine};

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
        .context("Failed to get datamine")?;

    fs::create_dir_all("export")
        .context("Failed to create export directory")?;

    for sheet in datamine.sheets() {
        export_sheet(&sheet)
            .with_context(|| format!("Failed to export sheet '{}'", sheet.title()))?;
    }

    let recipes = datamine.find_sheet_by_title("Recipes")
        .context("Failed to find recipes sheet")?;

    let recipes = recipes.json_rows()
        .context("Failed to get recipes json rows")?;

    let recipes_json = serde_json::to_vec_pretty(&recipes)
        .context("Failed to serialize recipes")?;

    fs::write("export/recipes.json", &recipes_json)
        .context("Failed to write recipes.json")?;

    Ok(())
}

fn export_sheet(sheet: &Sheet) -> Result<()> {
    let json_rows = sheet.json_rows()
        .context("Failed to convert to json rows")?;

    let json = serde_json::to_vec_pretty(&json_rows)
        .context("Failed to serialize to json")?;

    let filename = normalize_filename_fragment(sheet.title());
    let filename = format!("export/{}.json", filename);

    fs::write(&filename, &json)
        .with_context(|| format!("Failed to write {}", filename))?;

    Ok(())
}

/// Converts ' ' to '_' and strips all other non-alphanumeric characters, except '.'
fn normalize_filename_fragment(name: &str) -> String {
    name
    .chars()
    .flat_map(|c| Some(match c {
        c if c.is_ascii_alphanumeric() => c,
        '_' | '.' => c,
        ' ' => '_',
        _ => return None,
    }))
    .map(|c| c.to_ascii_lowercase())
    .collect()
}
