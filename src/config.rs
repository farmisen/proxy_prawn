use miette::Result;
use schematic::{Config, ConfigLoader};

#[derive(Config, Clone, Debug)]
pub struct AppConfig {
    #[setting(env = "OPENAI_API_KEY")]
    pub openai_api_key: String,

    #[setting(default = "127.0.0.1", env = "HOST")]
    pub host: String,

    #[setting(default = 3000, env = "PORT")]
    pub port: usize,

    #[setting(default = "https://api.openai.com/v1/")]
    pub openai_api_base_url: String,
}

pub fn load_config() -> miette::Result<AppConfig> {
    let result = ConfigLoader::new().load()?;

    Ok(result.config)
}
