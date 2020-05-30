use datamine_exporter::ExtendedValue;

fn main() {
    let datamine = datamine_exporter::read_datamine().unwrap();

    println!();
    println!("##############");
    println!("### Sheets ###");
    println!("##############");
    println!();

    for sheet in &datamine.sheets {
        println!("{}", sheet.properties.title);
    }

    for sheet in &datamine.sheets {
        if sheet.properties.title != "Recipes" {
            continue;
        }

        println!();
        println!("######################");
        println!("### Recipe columns ###");
        println!("######################");
        println!();

        let grid = sheet.data.first().unwrap();
        let row1 = grid.row_data.first().unwrap();

        for cell in &row1.values {
            if let ExtendedValue::String { value } = cell.effective_value.as_ref().unwrap() {
                println!("{}", value);
            }
        }
    }
}
