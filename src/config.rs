use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub admin_username: String,
    pub admin_password: String,
    pub jwt_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        // Load .env file if it exists, ignoring failure if missing
        let _ = dotenvy::dotenv();

        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/oxide_cms.db".to_string());
        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(3000);
        let admin_username = env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let admin_password = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin_secret_pass".to_string());
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "default_dev_secret_change_in_prod".to_string());

        Self {
            database_url,
            host,
            port,
            admin_username,
            admin_password,
            jwt_secret,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loads_defaults_or_env() {
        let config = AppConfig::from_env();
        assert!(config.database_url.contains("oxide_cms.db"));
        assert_eq!(config.port, 3000);
    }
}
