use anyhow::*;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct DataMine {
    pub sheets: Vec<Sheet>,
}

impl DataMine {
    pub fn find_sheet_by_title(&self, title: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|sheet| sheet.properties.title == title)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct Sheet {
    pub properties: SheetProperties,
    pub data: Vec<GridData>,
}

impl Sheet {
    pub fn column_titles(&self) -> Result<Vec<String>> {
        let titles = self.data.first().context("No grid data")?
            .row_data.first().context("No column titles")?
            .values.iter()
            .map(|cell| cell.to_string())
            .collect();

        Ok(titles)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct SheetProperties {
    pub title: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct GridData {
    pub row_data: Vec<RowData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct RowData {
    pub values: Vec<CellData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct CellData {
    pub user_entered_value: Option<ExtendedValue>,
    pub effective_value: Option<ExtendedValue>,
}

impl ToString for CellData {
    fn to_string(&self) -> String {
        if let Some(effective_value) = &self.effective_value {
            return match effective_value {
                ExtendedValue::String { value } => value.clone(),
                _ => unimplemented!("other extendend value types"),
            }
        }

        unimplemented!("effective_value is empty")
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
    Empty{},
}
