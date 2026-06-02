use crate::core::{Result, SkillKitsError, SkillMetadata};
use camino::{Utf8Path, Utf8PathBuf};

pub fn skill_markdown_path(skill_dir: &Utf8Path) -> Utf8PathBuf {
    skill_dir.join("SKILL.md")
}

pub fn disabled_skill_markdown_path(skill_dir: &Utf8Path) -> Utf8PathBuf {
    skill_dir.join("SKILL.md.disabled")
}

pub fn validate_skill_dir(skill_dir: &Utf8Path) -> Result<()> {
    if !skill_dir.is_dir() {
        return Err(SkillKitsError::InvalidSkillDir {
            path: skill_dir.to_path_buf(),
            reason: "path is not a directory".to_string(),
        });
    }

    let skill_md = skill_markdown_path(skill_dir);
    if !skill_md.is_file() {
        return Err(SkillKitsError::InvalidSkillDir {
            path: skill_dir.to_path_buf(),
            reason: "missing SKILL.md".to_string(),
        });
    }

    Ok(())
}

pub fn load_skill_metadata(skill_dir: &Utf8Path) -> Result<Option<SkillMetadata>> {
    load_skill_metadata_from_file(&skill_markdown_path(skill_dir))
}

pub fn load_skill_metadata_from_file(skill_file: &Utf8Path) -> Result<Option<SkillMetadata>> {
    let contents = std::fs::read_to_string(skill_file)?;
    Ok(parse_skill_metadata(&contents))
}

pub fn parse_skill_metadata(contents: &str) -> Option<SkillMetadata> {
    let (frontmatter, body) = split_frontmatter(contents);
    let mut table = toml::value::Table::new();

    if let Some(frontmatter) = frontmatter {
        if let Ok(parsed) = frontmatter.parse::<toml::Value>() {
            if let Some(parsed_table) = parsed.as_table() {
                table = parsed_table.clone();
            }
        }
    }

    let title = table
        .get("title")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| heading_title(body));

    let description = table
        .get("description")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| obvious_description(body));

    if title.is_none() && description.is_none() && table.is_empty() {
        None
    } else {
        Some(SkillMetadata {
            title,
            description,
            frontmatter: table,
        })
    }
}

fn split_frontmatter(contents: &str) -> (Option<&str>, &str) {
    let trimmed = contents.trim_start_matches('\u{feff}');
    for delimiter in ["+++", "---"] {
        let Some(after_open) = trimmed.strip_prefix(delimiter) else {
            continue;
        };
        let after_open = after_open
            .strip_prefix("\r\n")
            .or_else(|| after_open.strip_prefix('\n'))
            .unwrap_or(after_open);
        let closing = format!("\n{delimiter}");
        if let Some(end) = after_open.find(&closing) {
            let frontmatter = &after_open[..end];
            let body_start = end + closing.len();
            let body = after_open[body_start..]
                .strip_prefix("\r\n")
                .or_else(|| after_open[body_start..].strip_prefix('\n'))
                .unwrap_or(&after_open[body_start..]);
            return (Some(frontmatter), body);
        }
    }

    (None, contents)
}

fn heading_title(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn obvious_description(contents: &str) -> Option<String> {
    let mut in_code_block = false;

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('-') || line.starts_with('*') || line.starts_with('>') {
            continue;
        }
        return Some(line.to_string());
    }

    None
}
