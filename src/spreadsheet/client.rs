use anyhow::*;
use reqwest::Url;
use std::{fs, path::*};
use super::Spreadsheet;
use indicatif::*;

const DOWNLOAD_BUF_CAPACITY: usize = 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct Client {
    api_key: String,
    cache_dir: PathBuf,
}

impl Client {
    pub fn new(api_key: impl Into<String>, cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            api_key: api_key.into(),
            cache_dir: cache_dir.into(),
        }
    }

    pub fn prepare_cache_path(&self, spreadsheet_id: &str) -> Result<PathBuf> {
        fs::create_dir_all(&self.cache_dir)
            .context("Failed to create cache directory")?;

        Ok(self.cache_dir.join(spreadsheet_id))
    }

    pub async fn get(&self, spreadsheet_id: &str, instrument: &impl Instrument) -> Result<Spreadsheet> {
        let data = self.get_raw(spreadsheet_id, instrument)
            .await
            .context("Failed to get spreadsheet")?;
        
        let spreadsheet = Spreadsheet::from_json_bytes(&data)
            .context("Failed to parse spreadshet")?;
        
        Ok(spreadsheet)
    }

    pub async fn get_raw(
        &self,
        spreadsheet_id: &str,
        instrument: &impl Instrument,
    ) -> Result<Vec<u8>> {
        let path = self.prepare_cache_path(spreadsheet_id)
            .context("Failed to get cache path")?;

        let data = if path.exists() {
            fs::read(path).context("Failed reading cached spreadsheet")?
        } else {
            let data = self.get_raw_uncached(spreadsheet_id, instrument).await
                .context("failed downloading spreadsheet")?;

            fs::write(path, &data).context("Failed writing spreadsheet to cache")?;

            data
        };

        Ok(data)
    }

    async fn get_raw_uncached(&self, spreadsheet_id: &str, instrument: &impl Instrument) -> Result<Vec<u8>> {
        let client = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .build()?;
        let mut url = Url::parse("https://sheets.googleapis.com/v4/spreadsheets/")?
            .join(spreadsheet_id)?;

        url.query_pairs_mut()
            .append_pair("includeGridData", "true")
            .append_pair("key", &self.api_key);

        instrument.starting_request();

        let mut response: reqwest::Response = client.get(url)
            .header("Accept", "application/json")
            .send()
            .await
            .context("API request failed")?
            .error_for_status()
            .context("API returned an error")?;

        let mut data = Vec::with_capacity(DOWNLOAD_BUF_CAPACITY); // 1 GB;

        while let Some(chunk) = response.chunk().await.context("chunk failed")? {
            instrument.received_bytes(chunk.len());
            data.extend_from_slice(&chunk);
        }

        instrument.request_finished();
        data.shrink_to_fit();

        Ok(data)
    }
}

pub trait Instrument {
    fn starting_request(&self);
    fn received_bytes(&self, amount: usize);
    fn request_finished(&self);
}

impl Instrument for ProgressBar {
    fn starting_request(&self) {
        self.set_style(indicatif::ProgressStyle::default_spinner());
        self.set_message("Sending API request");
        self.enable_steady_tick(50);
    }

    fn received_bytes(&self, amount: usize) {
        self.inc(amount as u64);

        let bytes_downloaded = HumanBytes(self.position());
        self.set_message(&format!("Downloaded {}", bytes_downloaded));
    }

    fn request_finished(&self) {
        self.finish_and_clear();
    }
}
