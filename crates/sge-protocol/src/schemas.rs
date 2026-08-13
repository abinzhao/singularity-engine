use schemars::JsonSchema;

use crate::{
    AdapterDocument, ArtifactDocument, ContractDocument, EvidenceDocument, MemoryDocument,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaFile {
    file_name: &'static str,
    contents: String,
}

impl SchemaFile {
    pub fn file_name(&self) -> &'static str {
        self.file_name
    }

    pub fn contents(&self) -> &str {
        &self.contents
    }
}

pub fn v1_schema_files() -> Result<Vec<SchemaFile>, serde_json::Error> {
    Ok(vec![
        schema_file::<ArtifactDocument>("artifact.schema.json")?,
        schema_file::<ContractDocument>("contract.schema.json")?,
        schema_file::<EvidenceDocument>("evidence.schema.json")?,
        schema_file::<MemoryDocument>("memory.schema.json")?,
        schema_file::<AdapterDocument>("adapter.schema.json")?,
    ])
}

fn schema_file<T>(file_name: &'static str) -> Result<SchemaFile, serde_json::Error>
where
    T: JsonSchema,
{
    let schema = schemars::schema_for!(T);
    let mut contents = serde_json::to_string_pretty(&schema)?;
    contents.push('\n');

    Ok(SchemaFile {
        file_name,
        contents,
    })
}
