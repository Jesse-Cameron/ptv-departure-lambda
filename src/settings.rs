use config::{Config, ConfigError};
use serde::{de, Deserialize, Deserializer};

#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
pub struct Settings {
    pub uri: String,
    pub api_key: String,
    #[serde(deserialize_with = "de_u32_from_str")]
    pub developer_id: u32,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let cfg = Config::builder()
            .set_default("uri", "http://timetableapi.ptv.vic.gov.au")?
            // add in settings from the environment (with a prefix of APP)
            .add_source(config::Environment::with_prefix("APP"))
            .build()?;

        cfg.try_deserialize()
    }
}

fn de_u32_from_str<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        return Err(de::Error::custom("field is empty"));
    }
    s.parse::<u32>().map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_settings_deserialization() {
        // we just have a single test case here, as our tests are run
        // in parallel and we don't want the env vars mutation to effect
        // other tests

        let expected_output = Settings {
            uri: String::from("http://timetableapi.ptv.vic.gov.au"),
            api_key: String::from("my_api_key"),
            developer_id: 1234,
        };

        std::env::set_var("APP_API_KEY", "my_api_key");
        std::env::set_var("APP_DEVELOPER_ID", "1234");

        let settings = Settings::new().expect("Failed to deserialize settings");

        assert_eq!(settings, expected_output);

        // set the APP_DEVELOPER_ID to be empty and expect an error
        std::env::set_var("APP_DEVELOPER_ID", "");

        let res = Settings::new();
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "field is empty");

        std::env::remove_var("APP_API_KEY");
        std::env::remove_var("APP_DEVELOPER_ID");
    }
}
