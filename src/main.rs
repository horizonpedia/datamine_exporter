use std::collections::*;
use std::path::*;
use std::{ops, sync::*};
use anyhow::*;
use datamine_exporter::*;
use futures::prelude::*;
use indicatif::{ProgressBar, MultiProgress, HumanBytes};
use structopt::StructOpt;
use serde_json::{Value, Map};
use tokio::fs;

const CACHE_DIR: &str = "cache";
const EXPORT_DIR: &str = "export";
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
        eprintln!("{}", err);
    }

    Ok(())
}

async fn run(api_key: &str, opt: &Opt) -> Result<()> {
    let client = spreadsheet::Client::new(api_key, CACHE_DIR);

    eprintln!(">> Getting datamine");
    let datamine = client.get(DATAMINE_SHEET_ID, &new_spreadsheet_download_progress("datamine"))
        .await
        .context("Failed to get datamine")?;

    eprintln!(">> Transforming datamine");
    let mut datamine = JsonSheet::all_from_spreadsheet(datamine)
        .map(Datamine)
        .context("Failed to convert datamine to json sheets")?;

    eprintln!(">> Getting translations");
    let translations = client.get(TRANSLATIONS_SHEET_ID, &new_spreadsheet_download_progress("translations"))
        .await
        .context("Failed to get translations")?;

    datamine.assign_filenames_to_recipes()
        .context("Failed to assign filenames to recipes")?;

    fs::create_dir_all(EXPORT_DIR)
        .await
        .context("Failed to create export directory")?;

    let ref multi_progress = Arc::new(MultiProgress::new());
    let total_progress = multi_progress.add(ProgressBar::new(datamine.len() as u64));
    total_progress.enable_steady_tick(500);
    total_progress.set_style(PROGRESSBAR_STYLE.clone());

    tokio::task::spawn_blocking({
        let multi_progress = multi_progress.clone();
        move || multi_progress.join().unwrap()
    });

    datamine.export(&total_progress, &multi_progress, opt.download_images).await
        .context("Failed to export datamine")?;

    total_progress.finish_and_clear();

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

struct Datamine(pub BTreeMap<String, JsonSheet>);

impl Datamine {
    fn assign_filenames_to_recipes(&mut self) -> Result<()> {
        let mut recipes = self.remove("Recipes")
            .context("Failed to find recipes")?;

        // TODO: Make faster by creating a lookup: category => name => [filenames]
        for recipe in &mut recipes.rows {
            let category = recipe.get("category")
                .context("Failed to get category field for a recipe")?
                .as_str()
                .context("Category is not a string")?;

            let recipe_name = recipe.get("name")
                .context("Failed to get name field for a recipe")?
                .as_str()
                .context("Recipe name is not a string")?;

            let items = match self.get(category) {
                Some(items) => items,
                None => {
                    eprintln!(
                        "Warning: Skipping recipe '{}': Sheet/Category not found: {}",
                        recipe_name,
                        category,
                    );
                    continue;
                },
            };

            let mut filenames = Vec::new();

            for item in &items.rows {
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

        self.insert("Recipes".into(), recipes);

        Ok(())
    }

    async fn export(&self, total_progress: &ProgressBar, multi_progress: &MultiProgress, with_images: bool) -> Result<()> {
        for (title, sheet) in &**self {
            if title == "Read Me" {
                total_progress.inc(1);
                continue;
            }

            total_progress.set_message(&format!("Processing '{}'", title));

            sheet.export_to_dir(EXPORT_DIR).await
                .with_context(|| format!("Failed to export sheet '{}'", title))?;

            if with_images {
                sheet.download_images_to_dir(multi_progress).await
                    .with_context(|| format!("Failed to download images for sheet '{}'", title))?;
            }

            total_progress.inc(1);
        }

        Ok(())
    }
}

impl ops::Deref for Datamine {
    type Target = BTreeMap<String, JsonSheet>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Datamine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

type Row = Map<String, Value>;

struct JsonSheet {
    pub title: String,
    pub rows: Vec<Row>,
}

impl JsonSheet {
    fn all_from_spreadsheet(spreadsheet: Spreadsheet) -> Result<BTreeMap<String, Self>> {
        spreadsheet
        .sheets()
        .map(|sheet| sheet.json_rows().map(|rows| {
            let title = sheet.title().to_owned();
            let sheet = Self {
                title: title.clone(),
                rows,
            };

            (title, sheet)
        }))
        .collect::<Result<BTreeMap<_, _>>>()
        .context("Failed to convert datasheet to json rows")
    }

    async fn export_to_dir(&self, dir: impl AsRef<Path>) -> Result<()> {
        let filename = normalize_filename_fragment(&self.title);
        let filename = format!("{}.json", filename);
        let path = dir.as_ref().join(filename);

        self.export_to(path).await
    }

    async fn export_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        let json = serde_json::to_vec_pretty(&self.rows)
            .context("Failed to serialize to json")?;

        safe_write(path, &json).await
            .with_context(|| format!("Failed to write {}", path.display()))?;

        Ok(())
    }

    // TODO: move this function to Datamine struct
    async fn download_images_to_dir(&self, multi_progress: &MultiProgress) -> Result<()> {
        let required_fields_exist = self.rows.iter()
            .all(|row| row.get("image").is_some() && row.get("filename").is_some());

        if !required_fields_exist {
            return Ok(());
        }

        let ref total_progress = multi_progress.add(ProgressBar::new(self.rows.len() as u64));
        total_progress.set_style(PROGRESSBAR_STYLE_ETA.clone());
        total_progress.set_message("Downloading images");
        total_progress.enable_steady_tick(150);

        let dir = normalize_filename_fragment(&self.title);
        let ref dir = format!("{}/{}", IMAGE_EXPORT_PATH, dir);
        fs::create_dir_all(&dir).await?;

        stream::iter(&self.rows).map(Ok)
            .try_for_each_concurrent(10, move |row: &Map<String, Value>| async move {
                let result = download_image_for_row(dir, &row, multi_progress).await;
                total_progress.inc(1);
                result
            })
            .await?;

        total_progress.finish_and_clear();

        Ok(())
    }
}

fn new_spreadsheet_download_progress(sheet_name: &str) -> impl spreadsheet::client::Instrument + '_ {
    spreadsheet::client::FnInstrument {
        this: ProgressBar::new_spinner(),
        starting_request: move |this| {
            this.set_style(indicatif::ProgressStyle::default_spinner());
            this.set_message(&format!("Sending API request for {}", sheet_name));
            this.enable_steady_tick(50);
        },
        received_bytes: move |this, amount| {
            this.inc(amount as u64);

            let bytes_downloaded = HumanBytes(this.position());
            this.set_message(&format!("Downloaded {} of {}", bytes_downloaded, sheet_name));
        },
        request_finished: |this| {
            this.finish_and_clear();
        },
    }
}
