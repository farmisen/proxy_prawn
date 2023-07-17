use miette::Result;
use schematic::{Config, ConfigLoader};

#[derive(Config, Clone, Debug)]
pub struct AppConfig {
    #[setting(env = "OPENAI_API_KEY")]
    pub openai_api_key: String,
}

pub fn load_config() -> miette::Result<AppConfig> {
    let result = ConfigLoader::new().load()?;

    Ok(result.config)
}
