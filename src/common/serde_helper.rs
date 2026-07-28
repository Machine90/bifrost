use serde::Deserialize;

pub(crate) fn deserialize_option_str<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(value))
}
