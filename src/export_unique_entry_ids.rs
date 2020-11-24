use std::fs::{self, File};
use std::io::Write;

use anyhow::*;
use crate::{Datamine, EXPORT_DIR, Opt};

pub(crate) fn export_unique_entry_ids(opt: &Opt, datamine: &Datamine) -> Result<()> {
    fs::create_dir_all(EXPORT_DIR).context("Failed to create export dir")?;
    let mut file = File::create("export/unique_entry_ids.txt")
        .context("Failed to create `export/unique_entry_ids.txt`")?;

    let id_prefix = opt.id_prefix.as_deref().unwrap_or("");
    let id_suffix = opt.id_suffix.as_deref().unwrap_or("");
    
    for (sheet_title, sheet) in datamine.iter() {
        writeln!(file, "{}", sheet_title)?;

        for row in &sheet.rows {
            let id = match row.get("unique_entry_id") {
                Some(id) => id,
                None => continue,
            };
            let id = id.as_str().context("Invalid ID format")?;

            writeln!(file, "   {}{}{}", id_prefix, id, id_suffix)?;
        }
    }

    Ok(())
}
