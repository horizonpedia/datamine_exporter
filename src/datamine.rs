use anyhow::*;
use serde::Deserialize;
use serde_json as json;

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct DataMine {
    sheets: Vec<Sheet>,
}

impl DataMine {
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
    fn title(&self) -> &str {
        &self.properties.title
    }

    fn grid_data(&self) -> Result<&GridData> {
        let grid_data = self.data.first().context("No grid data")?;
        Ok(grid_data)
    }

    pub fn column_titles(&self) -> Result<Vec<String>> {
        self.grid_data()?
            .row_data.first().context("No column titles")?
            .values.iter()
            .map(|cell| cell.to_string().context("Empty column title"))
            .collect()
    }

    pub fn json_rows(&self) -> Result<Vec<json::Map<String, json::Value>>> {
        let columns = self.column_titles()?;
        let rows = self.grid_data()?
            .row_data.iter().skip(1)
            .map(|row| {
                let mut map = json::Map::new();

                for (i, cell) in row.values.iter().enumerate() {
                    let key = columns.get(i).cloned().unwrap_or_default();
                    let value = cell.to_string()
                        .map(json::Value::String)
                        .unwrap_or(json::Value::Null);

                    println!("Key: {}", key);
                    println!("Value: {}", value);

                    map.insert(key, value.into());
                }

                map
            })
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
    values: Vec<CellData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct CellData {
    user_entered_value: Option<ExtendedValue>,
    effective_value: Option<ExtendedValue>,
}

impl CellData {
    fn to_string(&self) -> Option<String> {
        if let Some(effective_value) = &self.effective_value {
            return Some(match effective_value {
                ExtendedValue::String { value } => value.clone(),
                ExtendedValue::Number { value } => value.to_string(),
                _ => unimplemented!("other effective value type: {:?}", effective_value),
            })
        }

        if let Some(user_entered_value) = &self.user_entered_value {
            return match user_entered_value {
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
    Empty{},
}
