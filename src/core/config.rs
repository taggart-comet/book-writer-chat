use std::{
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Component, Path, PathBuf},
};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEnvironment {
    Development,
    Test,
    Production,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: AppEnvironment,
    pub bind_addr: SocketAddr,
    pub data_dir: PathBuf,
    pub books_root: PathBuf,
    pub frontend_dist_dir: PathBuf,
    pub frontend_base_url: String,
    pub telegram_bot_username: String,
    pub max_bot_handle: String,
    pub reader_token_secret: String,
    pub codex_cli_path: String,
    pub codex_cli_args: Vec<String>,
    pub agent_timeout_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let cwd = env::current_dir().context("failed to resolve current working directory")?;
        let environment = match env::var("APP_ENV")
            .unwrap_or_else(|_| "development".to_string())
            .to_lowercase()
            .as_str()
        {
            "test" => AppEnvironment::Test,
            "production" => AppEnvironment::Production,
            _ => AppEnvironment::Development,
        };
        let defaults = environment.defaults();

        let bind_host = env::var("APP_HOST")
            .unwrap_or_else(|_| defaults.host.to_string())
            .parse::<IpAddr>()
            .with_context(|| "APP_HOST must be a valid IPv4 or IPv6 address")?;
        let bind_port = env::var("APP_PORT")
            .ok()
            .map(|value| {
                value
                    .parse::<u16>()
                    .with_context(|| "APP_PORT must be a valid TCP port")
            })
            .transpose()?
            .unwrap_or(defaults.port);
        let bind_addr = SocketAddr::new(bind_host, bind_port);

        let data_dir = resolve_path(
            &cwd,
            &env::var("APP_DATA_DIR").unwrap_or_else(|_| defaults.data_dir.to_string()),
        );
        let books_root = resolve_path(
            &cwd,
            &env::var("APP_BOOKS_ROOT").unwrap_or_else(|_| defaults.books_root.to_string()),
        );
        let frontend_dist_dir = resolve_path(
            &cwd,
            &env::var("FRONTEND_DIST_DIR")
                .unwrap_or_else(|_| defaults.frontend_dist_dir.to_string()),
        );

        let frontend_base_url =
            env::var("FRONTEND_BASE_URL").unwrap_or_else(|_| format!("http://{}", bind_addr));
        let reader_token_secret =
            env::var("READER_TOKEN_SECRET").unwrap_or_else(|_| "dev-reader-secret".to_string());

        if matches!(environment, AppEnvironment::Production) {
            anyhow::ensure!(
                !frontend_base_url.trim().is_empty(),
                "FRONTEND_BASE_URL is required in production so generated reader links use the public site URL"
            );
            anyhow::ensure!(
                reader_token_secret != "dev-reader-secret",
                "READER_TOKEN_SECRET must be set explicitly in production"
            );
        }

        Ok(Self {
            environment,
            bind_addr,
            data_dir,
            books_root,
            frontend_dist_dir,
            frontend_base_url,
            telegram_bot_username: env::var("TELEGRAM_BOT_USERNAME")
                .unwrap_or_else(|_| "bookbot".to_string()),
            max_bot_handle: env::var("MAX_BOT_HANDLE").unwrap_or_else(|_| "bookbot".to_string()),
            reader_token_secret,
            codex_cli_path: env::var("CODEX_CLI_PATH").unwrap_or_else(|_| "codex".to_string()),
            codex_cli_args: env::var("CODEX_CLI_ARGS")
                .unwrap_or_default()
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect(),
            agent_timeout_secs: env::var("AGENT_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(60),
        })
    }

    pub fn ensure_directories(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.books_root)?;
        Ok(())
    }
}

fn resolve_path(cwd: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        normalize_relative_path(cwd, &path)
    }
}

impl AppEnvironment {
    fn defaults(&self) -> EnvironmentDefaults {
        match self {
            Self::Development => EnvironmentDefaults {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 3000,
                data_dir: "data",
                books_root: "books-data",
                frontend_dist_dir: "frontend/build",
            },
            Self::Test => EnvironmentDefaults {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 3100,
                data_dir: "target/test/data",
                books_root: "target/test/books-data",
                frontend_dist_dir: "frontend/build",
            },
            Self::Production => EnvironmentDefaults {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 3000,
                data_dir: "/var/lib/book-writer-chat/state",
                books_root: "/var/lib/book-writer-chat/books-data",
                frontend_dist_dir: "/app/frontend/build",
            },
        }
    }
}

struct EnvironmentDefaults {
    host: IpAddr,
    port: u16,
    data_dir: &'static str,
    books_root: &'static str,
    frontend_dist_dir: &'static str,
}

fn normalize_relative_path(base: &Path, relative: &Path) -> PathBuf {
    let mut normalized = base.to_path_buf();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                if normalized != base {
                    normalized.pop();
                }
            }
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::sync::MutexGuard;

    use super::*;

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn clear_env() {
        for key in [
            "APP_ENV",
            "APP_HOST",
            "APP_PORT",
            "APP_BOOKS_ROOT",
            "APP_DATA_DIR",
            "FRONTEND_DIST_DIR",
            "FRONTEND_BASE_URL",
            "READER_TOKEN_SECRET",
        ] {
            unsafe { env::remove_var(key) };
        }
    }

    #[test]
    fn loads_development_defaults() {
        let _guard = env_lock();
        clear_env();

        let config = Config::from_env().unwrap();

        assert_eq!(config.environment, AppEnvironment::Development);
        assert_eq!(config.bind_addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(config.bind_addr.port(), 3000);
        assert!(config.books_root.ends_with("books-data"));
        assert!(config.data_dir.ends_with("data"));
    }

    #[test]
    fn loads_test_environment_and_uses_test_defaults() {
        let _guard = env_lock();
        clear_env();
        unsafe { env::set_var("APP_ENV", "test") };

        let config = Config::from_env().unwrap();

        assert_eq!(config.environment, AppEnvironment::Test);
        assert_eq!(config.bind_addr.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(config.bind_addr.port(), 3100);
        assert!(config.books_root.ends_with("target/test/books-data"));
        assert!(config.data_dir.ends_with("target/test/data"));
    }

    #[test]
    fn production_env_supports_explicit_bind_and_runtime_paths() {
        let _guard = env_lock();
        clear_env();
        unsafe {
            env::set_var("APP_ENV", "production");
            env::set_var("APP_HOST", "0.0.0.0");
            env::set_var("APP_PORT", "4300");
            env::set_var("FRONTEND_BASE_URL", "https://books.example.com");
            env::set_var("READER_TOKEN_SECRET", "prod-secret");
        }

        let config = Config::from_env().unwrap();

        assert_eq!(config.environment, AppEnvironment::Production);
        assert_eq!(config.bind_addr, "0.0.0.0:4300".parse().unwrap());
        assert_eq!(config.frontend_base_url, "https://books.example.com");
        assert_eq!(
            config.data_dir,
            PathBuf::from("/var/lib/book-writer-chat/state")
        );
        assert_eq!(
            config.books_root,
            PathBuf::from("/var/lib/book-writer-chat/books-data")
        );
        assert_eq!(
            config.frontend_dist_dir,
            PathBuf::from("/app/frontend/build")
        );
    }

    #[test]
    fn production_requires_non_default_reader_secret() {
        let _guard = env_lock();
        clear_env();
        unsafe {
            env::set_var("APP_ENV", "production");
            env::set_var("FRONTEND_BASE_URL", "https://books.example.com");
        }

        let error = Config::from_env().unwrap_err();

        assert!(
            error
                .to_string()
                .contains("READER_TOKEN_SECRET must be set explicitly in production")
        );
    }

    #[test]
    fn resolves_relative_books_root_inside_workspace_boundary() {
        let _guard = env_lock();
        clear_env();
        unsafe {
            env::set_var("APP_ENV", "test");
            env::set_var("APP_BOOKS_ROOT", "books-data/../books-data/session-root");
        }

        let cwd = env::current_dir().unwrap();
        let config = Config::from_env().unwrap();

        assert_eq!(config.books_root, cwd.join("books-data/session-root"));
        assert!(config.books_root.starts_with(&cwd));
    }
}
