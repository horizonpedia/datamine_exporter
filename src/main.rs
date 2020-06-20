use std::{collections::BTreeMap, sync::Arc, path::Path};
use anyhow::*;
use datamine_exporter::*;
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
    let datamine = get_datamine(DATAMINE_PATH, api_key)
        .await
        .context("Failed to get datamine")?;

    let mut sheets = datamine.sheets()
        .map(|sheet| sheet.json_rows().map(|rows| (sheet.title().to_owned(), rows)))
        .collect::<Result<BTreeMap<_, _>>>()
        .context("Failed to convert datasheet to json rows")?;

    drop(datamine);

    assign_filenames_to_recipes(&mut sheets)
        .context("Failed to assign filenames to recipes")?;

    fs::create_dir_all(EXPORT_PATH)
        .await
        .context("Failed to create export directory")?;

    let ref multi_progress = Arc::new(MultiProgress::new());
    let total_progress = multi_progress.add(ProgressBar::new(sheets.len() as u64));
    total_progress.enable_steady_tick(500);
    total_progress.set_style(PROGRESSBAR_STYLE.clone());

    tokio::task::spawn_blocking({
        let multi_progress = multi_progress.clone();
        move || multi_progress.join().unwrap()
    });

    for (title, rows) in sheets {
        if title == "Read Me" {
            total_progress.inc(1);
            continue;
        }

        total_progress.set_message(&format!("Processing '{}'", title));

        export_sheet(&title, &rows).await
            .with_context(|| format!("Failed to export sheet '{}'", title))?;

        if opt.download_images {
            download_images(&title, &rows, multi_progress).await
                .with_context(|| format!("Failed to download images for sheet '{}'", title))?;
        }

        total_progress.inc(1);
    }

    total_progress.finish_and_clear();

    Ok(())
}

async fn export_sheet(title: &str, rows: &[Map<String, Value>]) -> Result<()> {
    let json = serde_json::to_vec_pretty(&rows)
        .context("Failed to serialize to json")?;

    let filename = normalize_filename_fragment(title);
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

async fn download_images(title: &str, rows: &[Map<String, Value>], multi_progress: &MultiProgress) -> Result<()> {
    let required_fields_exist = rows.iter()
        .all(|row| row.get("image").is_some() && row.get("filename").is_some());

    if !required_fields_exist {
        return Ok(());
    }

    let ref total_progress = multi_progress.add(ProgressBar::new(rows.len() as u64));
    total_progress.set_style(PROGRESSBAR_STYLE_ETA.clone());
    total_progress.set_message("Downloading images");
    total_progress.enable_steady_tick(150);

    let dir = normalize_filename_fragment(title);
    let ref dir = format!("{}/{}", IMAGE_EXPORT_PATH, dir);
    fs::create_dir_all(&dir).await?;

    stream::iter(rows).map(Ok)
        .try_for_each_concurrent(10, move |row: &Map<String, Value>| async move {
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

fn assign_filenames_to_recipes(sheets: &mut BTreeMap<String, Vec<Map<String, Value>>>) -> Result<()> {
    let mut recipes = sheets.remove("Recipes")
        .context("Failed to find recipes")?;

    // TODO: Make faster by creating a lookup: category => name => [filenames]
    for recipe in &mut recipes {
        let category = recipe.get("category")
            .context("Failed to get category field for a recipe")?
            .as_str()
            .context("Category is not a string")?;

        let recipe_name = recipe.get("name")
            .context("Failed to get name field for a recipe")?
            .as_str()
            .context("Recipe name is not a string")?;

        let items = sheets.get(category)
            .context("Sheet not found")?;

        let mut filenames = Vec::new();

        for item in items {
            let item_name = item.get("name")
                .context("Failed to get name field for an item")?
                .as_str()
                .context("Item name is not a string")?;

            if recipe_name == item_name {
                let filename = item.get("filename")
                    .context("Failed to get filename field for an item")?
                    .as_str()
                    .context("Item filename is not a string")?;
                let filename = Value::from(filename);

                filenames.push(filename);
            }
        }

        let filenames = Value::from(filenames);

        recipe.insert("filenames".into(), filenames);
    }

    sheets.insert("Recipes".into(), recipes);

    Ok(())
}
