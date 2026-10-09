use chrono::{DateTime, Utc};
use regex_lite::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::core::{ProviderError, RateWindow, UsageSnapshot};

static OPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<option\b[^>]*>").expect("option regex"));
static ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(name|value)\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("attribute regex")
});

#[derive(Deserialize)]
struct QuotaInfo {
    #[serde(rename = "type")]
    kind: String,
    #[serde(deserialize_with = "number_or_string")]
    current: f64,
    #[serde(deserialize_with = "number_or_string")]
    maximum: f64,
}

pub(super) fn number_or_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Numeric(f64),
        Text(String),
    }
    match Number::deserialize(d)? {
        Number::Numeric(value) => Ok(value),
        Number::Text(value) => value.parse().map_err(serde::de::Error::custom),
    }
}

#[derive(Deserialize)]
struct NextRefill {
    #[serde(rename = "type")]
    kind: String,
    next: Option<DateTime<Utc>>,
}

pub(super) fn parse(content: &str) -> Result<UsageSnapshot, ProviderError> {
    let mut options = HashMap::new();
    for tag in OPTION.find_iter(content) {
        let mut name = None;
        let mut value = None;
        for attribute in ATTRIBUTE.captures_iter(tag.as_str()) {
            let raw = attribute
                .get(2)
                .or_else(|| attribute.get(3))
                .unwrap()
                .as_str();
            match &attribute[1] {
                "name" => name = Some(decode_xml(raw)?),
                "value" => value = Some(decode_xml(raw)?),
                _ => {}
            }
        }
        if let (Some(name), Some(value)) = (name, value) {
            options.insert(name, value);
        }
    }

    let (used, limit) = if let Some(json) = options.get("quotaInfo") {
        let quota: QuotaInfo = serde_json::from_str(json).map_err(|_| invalid_quota())?;
        if quota.kind != "Available" {
            return Err(invalid_quota());
        }
        // JetBrains current is consumed quota; available is maximum - current.
        // The top-level totals include both the subscription and top-up pools.
        (quota.current, quota.maximum)
    } else {
        let number = |names: &[&str]| {
            names
                .iter()
                .find_map(|name| options.get(*name)?.parse::<f64>().ok())
        };
        (
            number(&["usedCredits", "used_credits", "creditsUsed"]).ok_or_else(invalid_quota)?,
            number(&[
                "creditLimit",
                "credit_limit",
                "creditsLimit",
                "monthlyLimit",
            ])
            .ok_or_else(invalid_quota)?,
        )
    };
    if !used.is_finite() || used < 0.0 || !limit.is_finite() || limit <= 0.0 {
        return Err(invalid_quota());
    }

    // `until` is quota validity, not necessarily the next monthly refill.
    let reset = options
        .get("nextRefill")
        .and_then(|json| serde_json::from_str::<NextRefill>(json).ok())
        .filter(|refill| refill.kind == "Known")
        .and_then(|refill| refill.next);
    Ok(UsageSnapshot::new(RateWindow::with_details(
        used / limit * 100.0,
        None,
        reset,
        None,
    ))
    .with_login_method("JetBrains AI (local cache)"))
}

fn invalid_quota() -> ProviderError {
    ProviderError::Parse(
        "JetBrains AI quota is unavailable or invalid. Refresh usage in AI Assistant and let the IDE save its configuration.".into(),
    )
}

// JetBrains writes JSON as an XML attribute, including numeric newline entities.
fn decode_xml(value: &str) -> Result<String, ProviderError> {
    let mut decoded = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        let end = rest.find(';').ok_or_else(invalid_quota)?;
        let entity = &rest[..end];
        let character = match entity {
            "quot" => '"',
            "apos" => '\'',
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    entity
                        .strip_prefix('#')
                        .and_then(|digits| digits.parse().ok())
                };
                code.and_then(char::from_u32).ok_or_else(invalid_quota)?
            }
        };
        decoded.push(character);
        rest = &rest[end + 1..];
    }
    decoded.push_str(rest);
    Ok(decoded)
}
