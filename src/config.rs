use std::collections::HashMap;
use std::fs;

use crate::log::ScopedLogger;

#[derive(Clone)]
pub struct Config {
    entries: HashMap<String, String>,
    namespace: Option<String>,
    pub logger: ScopedLogger,
}

impl Config {
    pub fn read(logger: ScopedLogger) -> Config {
        let mut config = HashMap::new();
        if let Ok(src) = fs::read_to_string("config") {
            let config_lines = src.lines();
            for line in config_lines {
                let line = line.trim();
                if line.starts_with("#") || line.is_empty() {
                    continue;
                }
                let Some(splitindex) = line.find(" ") else {
                    continue;
                };
                let key = &line[..splitindex];
                let value = &line[splitindex + 1..];
                config.insert(key.to_string(), value.to_string());
            }
            if config.keys().len() == 0 {
                logger.warn("Configuration is empty");
            }
        } else {
            logger.warn("Config file not found");
        };
        Config {
            entries: config,
            namespace: None,
            logger,
        }
    }

    pub fn get_or_default<T: std::str::FromStr>(&self, key: &str, default: T) -> T {
        if let Some(val) = self.get(key) {
            val
        } else {
            default
        }
    }

    pub fn get<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        let key = if let Some(namespace) = &self.namespace {
            format!("{namespace}.{key}")
        } else {
            key.to_string()
        };
        if let Some(val) = self.entries.get(&key) {
            if let Ok(val) = val.parse::<T>() {
                Some(val)
            } else {
                self.logger
                    .err(&format!("Invalid value {val} for key '{key}'"));
                None
            }
        } else {
            self.logger.warn(&format!("Missing config key '{key}'"));
            None
        }
    }

    pub fn reduce_to_namespace(&self, namespace: &str) -> Config {
        Config {
            entries: self.entries.clone(),
            namespace: Some(namespace.to_string()),
            logger: self.logger.clone(),
        }
    }
}
