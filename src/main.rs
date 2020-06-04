use std::{collections::BTreeMap, sync::Arc, path::Path};
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

    safe_write(&filename, &json).await
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

    let image_column_exists = titles.iter().any(|title| title == "image");
    let filename_column_exists = titles.iter().any(|title| title == "filename");

    if !image_column_exists || !filename_column_exists {
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
            let result = download_image_for_row(dir, &row, multi_progress).await;
            total_progress.inc(1);
            result
        })
        .await?;

    total_progress.finish_and_clear();

    Ok(())
}

async fn download_image_for_row<'a>(
    dir: &str,
    row: &Map<String, Value>,
    multi_progress: &MultiProgress,
) -> Result<()> {
    let image = match Image::from_row(row) {
        Some(image) => image,
        None => return Ok(()),
    };
    let download_path = format!("{}/{}.png", dir, image.filename);
    let file_exists = Path::new(&download_path).exists();

    if file_exists {
        return Ok(());
    }

    let progress = multi_progress.add(ProgressBar::new_spinner());
    progress.enable_steady_tick(150);
    progress.set_message(&format!("Downloading {}", download_path));

    image.download_to(download_path).await
        .with_context(|| image.url.to_string())?;

    progress.finish_and_clear();

    Ok(())
}

struct Image<'a> {
    url: &'a str,
    filename: &'a str,
}

impl<'a> Image<'a> {
    fn from_row(row: &'a Map<String, Value>) -> Option<Self> {
        Some(Self {
            url: row.get("image")?.as_str()?,
            filename: row.get("filename")?.as_str()?,
        })
    }

    async fn download(&self) -> Result<impl AsRef<[u8]>> {
        let response = reqwest::get(self.url).await
            .context("Failed to request image")?;

        let image = response.bytes().await
            .context("Failed to download image completely")?;

        Ok(image)
    }

    async fn download_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let image = self.download().await
            .context("Failed to download image")?;

        safe_write(&path, &image).await
            .with_context(|| format!("Failed to write {}", path.as_ref().display()))?;

        Ok(())
    }
}

async fn safe_write(path: impl AsRef<Path>, data: impl AsRef<[u8]> + Unpin) -> Result<()> {
    let path = path.as_ref();
    let file_name = path.file_name()
        .context("Path without filename was given")?
        .to_string_lossy();
    let file_name = format!("{}.tmp", file_name);
    let tmp_path = path.with_file_name(file_name);

    fs::write(&tmp_path, data).await
        .with_context(|| format!("Failed to write {}", tmp_path.display()))?;

    fs::rename(&tmp_path, &path).await
        .with_context(|| format!("Failed to move {} to {}", tmp_path.display(), path.display()))?;

    Ok(())
}
