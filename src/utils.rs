use serde::{Deserialize, Deserializer};

pub fn i64_from_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.parse::<i64>() {
        Ok(int) => Ok(int),
        Err(e) => Err(serde::de::Error::custom(e.to_string())),
    }
}
