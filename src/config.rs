//! Explicit, data-only configuration for editorial review.
use crate::rules::{Rule, Severity};
use anyhow::{Context, Result, bail};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use std::{collections::HashSet, fs, path::Path};

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub(crate) select: Option<Vec<String>>,
    pub(crate) ignore: Vec<String>,
    pub(crate) allow: Vec<String>,
    pub(crate) custom_rules: Vec<CustomRule>,
    #[serde(deserialize_with = "optional_severity")]
    pub(crate) severity: Option<Severity>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CustomRule {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) pattern: String,
    pub(crate) hint: String,
    #[serde(deserialize_with = "severity")]
    pub(crate) severity: Severity,
    #[serde(skip)]
    pub(crate) regex: Option<Regex>,
}
impl Config {
    pub(crate) fn load(path: &Path, available: &[&Rule]) -> Result<Self> {
        let data = fs::read_to_string(path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let mut config: Self = serde_json::from_str(&data)
            .with_context(|| format!("parsing config {}", path.display()))?;
        let mut ids: HashSet<&str> = available.iter().map(|r| r.id).collect();
        for rule in &mut config.custom_rules {
            let mut chars = rule.id.chars();
            if !chars.next().is_some_and(|c| c.is_ascii_lowercase())
                || !chars
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
            {
                bail!("invalid custom rule id `{}`: use [a-z][a-z0-9_-]*", rule.id);
            }
            if !ids.insert(&rule.id) {
                bail!("duplicate custom rule id `{}`", rule.id);
            }
            if rule.title.trim().is_empty() || rule.hint.trim().is_empty() {
                bail!("custom rule `{}` requires a title and hint", rule.id);
            }
            let regex = Regex::new(&rule.pattern)
                .with_context(|| format!("invalid pattern for custom rule `{}`", rule.id))?;
            if regex.is_match("") {
                bail!("custom rule `{}` must not match an empty string", rule.id);
            }
            rule.regex = Some(regex);
        }
        if config.allow.iter().any(String::is_empty) {
            bail!("allow phrases must not be empty");
        }
        Ok(config)
    }
    pub(crate) fn allows(&self) -> Result<Vec<Regex>> {
        self.allow
            .iter()
            .map(|s| {
                RegexBuilder::new(&regex::escape(s))
                    .case_insensitive(true)
                    .build()
                    .context("compiling allow phrase")
            })
            .collect()
    }
}

fn severity<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Severity, D::Error> {
    match String::deserialize(d)?.as_str() {
        "info" => Ok(Severity::Info),
        "warn" => Ok(Severity::Warn),
        _ => Err(serde::de::Error::custom("severity must be info or warn")),
    }
}
fn optional_severity<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Severity>, D::Error> {
    Option::<String>::deserialize(d)?
        .map(|s| match s.as_str() {
            "info" => Ok(Severity::Info),
            "warn" => Ok(Severity::Warn),
            _ => Err(serde::de::Error::custom("severity must be info or warn")),
        })
        .transpose()
}
