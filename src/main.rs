use std::{collections::BTreeMap, sync::Arc};
use anyhow::*;
use datamine_exporter::{Sheet, get_cached_or_download_datamine};
use futures::prelude::*;
use indicatif::{ProgressBar, MultiProgress};
use structopt::StructOpt;
use serde_json::{Value, Map};
use tokio::fs;
use datamine_exporter::{PROGRESSBAR_STYLE, PROGRESSBAR_STYLE_ETA};

const DATAMINE_PATH: &str = "datamine.json";
const EXPORT_PATH: &str = "export";
const IMAGE_EXPORT_PATH: &str = "export/images";

#[derive(StructOpt)]
#[structopt(
    name = "datamine_exporter",
    about = "Exports the AC:NH datamine into usable bits"
)]
struct Opt {
    #[structopt(long = "dl-images")]
    download_images: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let vars = dotenv::vars()
        .collect::<BTreeMap<String, String>>();

    let api_key = vars.get("API_KEY")
        .context("API_KEY missing in .env")?;

    if let Err(err) = run(api_key, &opt).await {
        let err = format!("{:?}", err).replace(api_key, "<REDACTED>");
        println!("{}", err);
    }

    Ok(())
}

async fn run(api_key: &str, opt: &Opt) -> Result<()> {
    let datamine = get_cached_or_download_datamine(DATAMINE_PATH, api_key)
        .await
        .context("Failed to get datamine")?;

    fs::create_dir_all(EXPORT_PATH)
        .await
        .context("Failed to create export directory")?;

    let ref multi_progress = Arc::new(MultiProgress::new());
    let total_progress = multi_progress.add(ProgressBar::new(datamine.sheets().count() as u64));
    total_progress.enable_steady_tick(500);
    total_progress.set_style(PROGRESSBAR_STYLE.clone());

    tokio::task::spawn_blocking({
        let multi_progress = multi_progress.clone();
        move || multi_progress.join().unwrap()
    });

    for sheet in datamine.sheets() {
        if sheet.title() == "Read Me" {
            total_progress.inc(1);
            continue;
        }

        total_progress.set_message(&format!("Processing '{}'", sheet.title()));

        export_sheet(&sheet).await
            .with_context(|| format!("Failed to export sheet '{}'", sheet.title()))?;

        if opt.download_images {
            download_images(sheet, multi_progress).await
                .with_context(|| format!("Failed to download images for sheet '{}'", sheet.title()))?;
        }

        total_progress.inc(1);
    }

    total_progress.finish_and_clear();

    Ok(())
}

async fn export_sheet(sheet: &Sheet) -> Result<()> {
    let json_rows = sheet.json_rows()
        .context("Failed to convert to json rows")?;

    let json = serde_json::to_vec_pretty(&json_rows)
        .context("Failed to serialize to json")?;

    let filename = normalize_filename_fragment(sheet.title());
    let filename = format!("{}/{}.json", EXPORT_PATH, filename);

    fs::write(&filename, &json).await
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

async fn download_images(sheet: &Sheet, multi_progress: &MultiProgress) -> Result<()> {
    let titles = sheet.column_titles()
        .context("Failed to get column titles")?;

    if !titles.iter().any(|title| title == "image") {
        return Ok(());
    }

    let json_rows = sheet.json_rows()
        .context("Failed to get json rows")?;

    let ref total_progress = multi_progress.add(ProgressBar::new(json_rows.len() as u64));
    total_progress.set_style(PROGRESSBAR_STYLE_ETA.clone());
    total_progress.set_message("Downloading images");
    total_progress.enable_steady_tick(150);

    let dir = normalize_filename_fragment(sheet.title());
    let ref dir = format!("{}/{}", IMAGE_EXPORT_PATH, dir);
    fs::create_dir_all(&dir).await?;

    stream::iter(json_rows).map(Ok)
        .try_for_each_concurrent(10, move |row: Map<String, Value>| async move {
            let filename = row.get("filename").and_then(|col| col.as_str());
            let filename = match filename {
                Some(filename) => filename,
                None => return Ok::<_, Error>(()),
            };
            let image_url = row.get("image").and_then(|col| col.as_str());
            let image_url = match image_url {
                Some(image_url) => image_url,
                None => return Ok::<_, Error>(()),
            };

            let filename = format!("{}/{}.png", dir, filename);

            let progress = multi_progress.add(ProgressBar::new_spinner());
            progress.enable_steady_tick(150);
            progress.set_message(&format!("Downloading {}", filename));

            let response = reqwest::get(image_url).await
                .context("Failed to request image")?;
            let image = response.bytes().await
                .context("Failed to download image completely")?;

            fs::write(&filename, &image).await
                .with_context(|| format!("Failed to write {}", filename))?;

            progress.finish_and_clear();
            total_progress.inc(1);

            Ok(())
        })
        .await?;

    total_progress.finish_and_clear();

    Ok(())
}
