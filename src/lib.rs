use lazy_static::lazy_static;
use indicatif::ProgressStyle;

pub mod spreadsheet;

pub const DATAMINE_SHEET_ID: &str = "13d_LAJPlxMa_DubPTuirkIV4DERBMXbrWQsmSh8ReK4";
pub const DOWNLOAD_BUF_CAPACITY: usize = 1024 * 1024 * 1024;

lazy_static! {
    pub static ref PROGRESSBAR_STYLE: ProgressStyle = ProgressStyle::default_bar()
        .template("{msg} [{elapsed_precise}] [{pos:}/{len}] {wide_bar}");

    pub static ref PROGRESSBAR_STYLE_ETA: ProgressStyle = ProgressStyle::default_bar()
        .template("{msg} [ETA {eta}] [{pos}/{len}] {wide_bar}");
}
