use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use crate::core::hash::DEFAULT_HASH_SUFFIX_LEN;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(SkillId);
id_type!(AgentId);
id_type!(ProjectId);

pub fn generate_skill_id(name: &str, content_hash: &str) -> SkillId {
    skill_id_with_suffix_len(name, content_hash, DEFAULT_HASH_SUFFIX_LEN)
}

pub fn unique_skill_id<'a, I>(name: &str, content_hash: &str, existing_ids: I) -> SkillId
where
    I: IntoIterator<Item = &'a SkillId>,
{
    let existing: HashSet<&str> = existing_ids.into_iter().map(SkillId::as_str).collect();
    let hash_len = content_hash.len().max(DEFAULT_HASH_SUFFIX_LEN);

    for suffix_len in (DEFAULT_HASH_SUFFIX_LEN..=hash_len).step_by(2) {
        let candidate = skill_id_with_suffix_len(name, content_hash, suffix_len);
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }

    let slug = normalize_skill_name(name);
    let full_hash = content_hash.trim();
    let mut index = 2;
    loop {
        let candidate = SkillId::new(format!("{slug}-{full_hash}-{index}"));
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
        index += 1;
    }
}

fn skill_id_with_suffix_len(name: &str, content_hash: &str, suffix_len: usize) -> SkillId {
    let slug = normalize_skill_name(name);
    let hash = content_hash.trim();
    let suffix = hash.get(..suffix_len.min(hash.len())).unwrap_or(hash);
    SkillId::new(format!("{slug}-{suffix}"))
}

pub fn normalize_skill_name(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "skill".to_string()
    } else {
        slug
    }
}
