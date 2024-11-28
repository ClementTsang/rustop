#![cfg(feature = "generate_schema")]

use bottom::options::config;
use bottom::widgets;
use itertools::Itertools;
use strum::VariantArray;

fn generate_schema() -> anyhow::Result<()> {
    let mut schema = schemars::schema_for!(config::Config);
    {
        // TODO: Maybe make this case insensitive? See https://stackoverflow.com/a/68639341

        let proc_columns = schema.definitions.get_mut("ProcColumn").unwrap();
        match proc_columns {
            schemars::schema::Schema::Object(proc_columns) => {
                let enums = proc_columns.enum_values.as_mut().unwrap();
                *enums = widgets::ProcColumn::VARIANTS
                    .iter()
                    .flat_map(|var| var.get_schema_names())
                    .sorted()
                    .map(|v| serde_json::Value::String(v.to_string()))
                    .dedup()
                    .collect();
            }
            _ => anyhow::bail!("missing proc columns definition"),
        }

        let disk_columns = schema.definitions.get_mut("DiskColumn").unwrap();
        match disk_columns {
            schemars::schema::Schema::Object(disk_columns) => {
                let enums = disk_columns.enum_values.as_mut().unwrap();
                *enums = widgets::DiskColumn::VARIANTS
                    .iter()
                    .flat_map(|var| var.get_schema_names())
                    .sorted()
                    .map(|v| serde_json::Value::String(v.to_string()))
                    .dedup()
                    .collect();
            }
            _ => anyhow::bail!("missing disk columns definition"),
        }
    }

    let metadata = schema.schema.metadata.as_mut().unwrap();
    metadata.id = Some(
        "https://github.com/ClementTsang/bottom/blob/main/schema/nightly/bottom.json".to_string(),
    );
    metadata.description =
        Some("https://clementtsang.github.io/bottom/nightly/configuration/config-file".to_string());
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());

    Ok(())
}

fn main() -> anyhow::Result<()> {
    generate_schema()?;

    Ok(())
}
