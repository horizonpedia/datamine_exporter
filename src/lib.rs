use std::fs;
use anyhow::*;
use serde::Deserialize;

const DATAMINE_PATH: &str = "datamine.json";

pub fn read_datamine() -> Result<DataMine> {
    let data = fs::read(DATAMINE_PATH)
        .context("open datamine file")?;

    let datamine = serde_json::from_slice::<DataMine>(&data)
        .context("deserializing datamine")?;

    Ok(datamine)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct DataMine {
    pub sheets: Vec<Sheet>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct Sheet {
    pub properties: SheetProperties,
    pub data: Vec<GridData>,
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
