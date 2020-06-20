use anyhow::*;
use reqwest::Url;
use std::{fs, path::*};
use super::Spreadsheet;

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

pub struct FnInstrument<T, F1, F2, F3>
where
    F1: Fn(&T),
    F2: Fn(&T, usize),
    F3: Fn(&T),
{
    pub this: T,
    pub starting_request: F1,
    pub received_bytes: F2,
    pub request_finished: F3,
}

impl<T, F1, F2, F3> Instrument for FnInstrument<T, F1, F2, F3>
where
    F1: Fn(&T),
    F2: Fn(&T, usize),
    F3: Fn(&T),
{
    fn starting_request(&self) {
        (self.starting_request)(&self.this)
    }

    fn received_bytes(&self, amount: usize) {
        (self.received_bytes)(&self.this, amount)
    }

    fn request_finished(&self) {
        (self.request_finished)(&self.this)
    }
}

pub trait Instrument {
    fn starting_request(&self);
    fn received_bytes(&self, amount: usize);
    fn request_finished(&self);
}
