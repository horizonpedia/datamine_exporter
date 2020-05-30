use std::fs;
use std::path::Path;
use anyhow::*;

mod datamine;
pub use datamine::*;
use reqwest::Url;
use indicatif::HumanBytes;

const DATAMINE_SHEET_ID: &str = "13d_LAJPlxMa_DubPTuirkIV4DERBMXbrWQsmSh8ReK4";
const DOWNLOAD_BUF_CAPACITY: usize = 1024 * 1024 * 1024;

pub async fn get_cached_or_download_datamine(path: impl AsRef<Path>, api_key: &str) -> Result<DataMine> {
    let path = path.as_ref();

    let data = if path.exists() {
        fs::read(path).context("failed reading cached datamine")?
    } else {
        let data = download_datamine(api_key).await.context("failed downloading datamine")?;

        fs::write(path, &data).context("failed writing datamine cache")?;

        data
    };

    let datamine = parse_datamine(&data).context("failed parsing datamine")?;

    Ok(datamine)
}

pub fn parse_datamine(data: &[u8]) -> Result<DataMine> {
    let datamine = serde_json::from_slice::<DataMine>(&data)
        .context("failed deserializing datamine")?;

    Ok(datamine)
}

async fn download_datamine(api_key: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .gzip(true)
        .brotli(true)
        .build()?;
    let mut url = Url::parse("https://sheets.googleapis.com/v4/spreadsheets/")?
        .join(DATAMINE_SHEET_ID)?;

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
