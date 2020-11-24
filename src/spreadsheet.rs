use anyhow::*;
use std::{borrow::Cow, ops::*};
use serde::Deserialize;
use serde_json as json;
use lazy_static::lazy_static;
use regex::Regex;

pub mod client;
pub use client::Client;

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct Spreadsheet {
    sheets: Vec<Sheet>,
}

impl Spreadsheet {
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        let spreadsheet = serde_json::from_slice::<Spreadsheet>(&bytes)
            .context("Failed deserializing spreadsheet")?;

        Ok(spreadsheet)
    }

    pub fn sheets(&self) -> impl Iterator<Item = &Sheet> {
        self.sheets.iter()
    }

    pub fn find_sheet_by_title(&self, title: &str) -> Option<&Sheet> {
        self.sheets().find(|sheet| sheet.title() == title)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct Sheet {
    properties: SheetProperties,
    data: Vec<GridData>,
}

impl Sheet {
    pub fn title(&self) -> &str {
        &self.properties.title
    }

    fn grid_data(&self) -> Result<&GridData> {
        let grid_data = self.data.first().context("No grid data")?;
        Ok(grid_data)
    }

    /// Returns the column titles (first row) in a normalized form:
    /// - lowercased
    /// - spaces are translated to `_`
    pub fn column_titles(&self) -> Result<Vec<String>> {
        self.grid_data()?
            .row_data.first().context("No column titles")?
            .values.iter()
            .map(|cell| cell
                .to_string()
                .map(|column| Self::normalize_column_name(&column))
                .context("Empty column title")
            )
            .collect()
    }

    fn normalize_column_name(column: &str) -> String {
        column
        .chars()
        .map(|c| match c {
            ' ' => '_',
            c => c.to_ascii_lowercase(),
        })
        .collect()
    }

    pub fn rows(&self) -> Result<&[RowData]> {
        let grid_data = self.grid_data()?;

        if grid_data.row_data.len() <= 1 {
            return Ok(&[]);
        }

        Ok(&grid_data.row_data[1..])
    }

    pub fn json_rows(&self) -> Result<Vec<json::Map<String, json::Value>>> {
        let columns = self.column_titles()?;
        let rows = self.grid_data()?
            .row_data.iter().skip(1)
            .map(|row| {
                let mut map = json::Map::new();

                for (i, cell) in row.values.iter().enumerate() {
                    let key = columns.get(i).cloned().unwrap_or_default();
                    // eprintln!("Key = {}", key);
                    let value = cell.to_string()
                        .map(Cow::into_owned)
                        .map(json::Value::String)
                        .unwrap_or(json::Value::Null);
                    // eprintln!("Value = {}", value);

                    map.insert(key, value.into());
                }

                map
            })
            // Only keep rows with data
            .filter(|row| row.iter().any(|(_key, value)| value != &json::Value::Null))
            .collect();

        Ok(rows)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct SheetProperties {
    title: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct GridData {
    row_data: Vec<RowData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct RowData {
    #[serde(default)]
    values: Vec<CellData>,
}

impl Deref for RowData {
    type Target = [CellData];

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

lazy_static! {
    static ref IMAGE_FORMULA_RE: Regex =
        Regex::new(r#"(?i)=IMAGE\("(.*)"\)"#).unwrap();
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct CellData {
    user_entered_value: Option<ExtendedValue>,
    effective_value: Option<ExtendedValue>,
}

impl CellData {
    pub fn to_string(&self) -> Option<Cow<str>> {
        if let Some(effective_value) = &self.effective_value {
            match effective_value {
                ExtendedValue::String { value } => return Some(Cow::Borrowed(value)),
                ExtendedValue::Number { value } => return Some(value.to_string().into()),
                // In this case, fall back to user_entered_value
                ExtendedValue::Empty {} => {},
                _ => unimplemented!("other effective value type: {:?}", effective_value),
            }
        }

        if let Some(user_entered_value) = &self.user_entered_value {
            match user_entered_value {
                ExtendedValue::Formula { value } => {
                    // eprintln!("Formula: {}", value);
                    let caps = IMAGE_FORMULA_RE.captures(value).expect("image formula expected");
                    let image_url = caps.get(1).unwrap().as_str();

                    return Some(Cow::Owned(image_url.to_string()));
                },
                ExtendedValue::Empty {} => return None,
                _ => unimplemented!("other user entered value type: {:?}", user_entered_value),
            }
        }

        None
    }
}

// #[derive(Deserialize, Debug)]
// #[serde(rename_all="camelCase")]
// pub struct ExtendedValue {
//     number_value: Option<f64>,
//     // bool_value: Option<bool>,
//     // // error_value: Option<f64>,
//     // formula_value: Option<String>,
//     // string_value: Option<String>,
// }

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ExtendedValue {
    Number {
        #[serde(rename="numberValue")]
        value: f64,
    },
    String {
        #[serde(rename="stringValue")]
        value: String,
    },
    Bool {
        #[serde(rename="boolValue")]
        value: bool,
    },
    Formula {
        #[serde(rename="formulaValue")]
        value: String,
    },
    // error_value: Option<ErrorValue>,
    Empty {},
}
