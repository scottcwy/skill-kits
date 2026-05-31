use serde::Serialize;

pub fn to_json<T: Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableColumn(String);

impl TableColumn {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TableColumn {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for TableColumn {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<usize> for TableColumn {
    fn from(value: usize) -> Self {
        Self(value.to_string())
    }
}

impl From<bool> for TableColumn {
    fn from(value: bool) -> Self {
        Self(value.to_string())
    }
}

pub fn format_table(headers: &[&str], rows: &[Vec<TableColumn>]) -> String {
    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL);
    table.set_header(headers.iter().copied());

    for row in rows {
        table.add_row(row.iter().map(TableColumn::as_str));
    }

    table.to_string()
}
