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
        // Test with custom env vars
        unsafe {
            std::env::set_var("DATABASE_URL", "sqlite:custom.db");
            std::env::set_var("HOST", "0.0.0.0");
            std::env::set_var("PORT", "8080");
            std::env::set_var("ADMIN_USERNAME", "custom_admin");
            std::env::set_var("ADMIN_PASSWORD", "custom_pass");
            std::env::set_var("JWT_SECRET", "custom_secret");
        }
        let custom_cfg = AppConfig::from_env();
        assert_eq!(custom_cfg.host, "0.0.0.0");
        assert_eq!(custom_cfg.port, 8080);
        assert_eq!(custom_cfg.admin_username, "custom_admin");
        assert_eq!(custom_cfg.admin_password, "custom_pass");
        assert_eq!(custom_cfg.jwt_secret, "custom_secret");
        assert_eq!(custom_cfg.database_url, "sqlite:custom.db");

        // Test with invalid PORT to hit .and_then() None branch
        unsafe {
            std::env::set_var("PORT", "invalid_port");
        }
        let invalid_port_cfg = AppConfig::from_env();
        assert_eq!(invalid_port_cfg.port, 3000);

        unsafe {
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("HOST");
            std::env::remove_var("PORT");
            std::env::remove_var("ADMIN_USERNAME");
            std::env::remove_var("ADMIN_PASSWORD");
            std::env::remove_var("JWT_SECRET");
        }
        // Test fallback to defaults or .env
        let config = AppConfig::from_env();
        assert!(config.database_url.contains("oxide_cms.db") || config.database_url.contains("data"));
    }
}
