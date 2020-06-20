use std::fs;
use std::path::Path;
use anyhow::*;

mod spreadsheet;
pub use spreadsheet::*;
use reqwest::Url;
use lazy_static::lazy_static;
use indicatif::{HumanBytes, ProgressStyle};

const DATAMINE_SHEET_ID: &str = "13d_LAJPlxMa_DubPTuirkIV4DERBMXbrWQsmSh8ReK4";
const DOWNLOAD_BUF_CAPACITY: usize = 1024 * 1024 * 1024;

lazy_static! {
    pub static ref PROGRESSBAR_STYLE: ProgressStyle = ProgressStyle::default_bar()
        .template("{msg} [{elapsed_precise}] [{pos:}/{len}] {wide_bar}");

    pub static ref PROGRESSBAR_STYLE_ETA: ProgressStyle = ProgressStyle::default_bar()
        .template("{msg} [ETA {eta}] [{pos}/{len}] {wide_bar}");
}

pub async fn get_datamine(path: impl AsRef<Path>, api_key: &str) -> Result<Spreadsheet> {
    let path = path.as_ref();

    let data = if path.exists() {
        fs::read(path).context("Failed reading cached datamine")?
    } else {
        let data = download_spreadsheet(DATAMINE_SHEET_ID, api_key).await
            .context("failed downloading datamine")?;

        fs::write(path, &data).context("Failed writing datamine cache")?;

        data
    };

    let datamine = Spreadsheet::from_json_bytes(&data)?;

    Ok(datamine)
}

pub async fn get_cached_or_download_spreadsheet(
    path: impl AsRef<Path>,
    spreadsheet_id: &str,
    api_key: &str,
) -> Result<Spreadsheet> {
    let path = path.as_ref();

    let data = if path.exists() {
        fs::read(path).context("Failed reading cached spreadsheet")?
    } else {
        let data = download_spreadsheet(spreadsheet_id, api_key).await
            .context("failed downloading spreadsheet")?;

        fs::write(path, &data).context("Failed writing datamine cache")?;

        data
    };

    let spreadsheet = Spreadsheet::from_json_bytes(&data)?;

    Ok(spreadsheet)
}

async fn download_spreadsheet(
    spreadsheet_id: &str,
    api_key: &str,
) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .gzip(true)
        .brotli(true)
        .build()?;
    let mut url = Url::parse("https://sheets.googleapis.com/v4/spreadsheets/")?
        .join(spreadsheet_id)?;

    url.query_pairs_mut()
        .append_pair("includeGridData", "true")
        .append_pair("key", api_key);

    let spinner = indicatif::ProgressBar::new_spinner();

    spinner.set_message("Sending API request");
    spinner.enable_steady_tick(50);

    let mut response: reqwest::Response = client.get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .context("API request failed")?
        .error_for_status()
        .context("API returned an error")?;

    let mut data = Vec::with_capacity(DOWNLOAD_BUF_CAPACITY); // 1 GB;

    while let Some(chunk) = response.chunk().await.context("chunk failed")? {
        data.extend_from_slice(&chunk);
        spinner.inc(chunk.len() as u64);

        let bytes_downloaded = HumanBytes(spinner.position());
        spinner.set_message(&format!("Downloaded {}", bytes_downloaded));
    }

    spinner.finish();
    data.shrink_to_fit();

    Ok(data)
}
