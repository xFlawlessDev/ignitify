use std::fs;
use std::path::Path;

const ENV_KEYS: [&str; 3] = [
    "IGNITIFY_DATABASE_URL",
    "IGNITIFY_JWT_SECRET",
    "IGNITIFY_SECURE_COOKIES",
];

fn main() {
    println!("cargo:rerun-if-changed=../../.env");
    println!("cargo:rerun-if-changed=../../.env.example");

    for key in ENV_KEYS {
        println!("cargo:rerun-if-env-changed={key}");
        if let Some(value) = std::env::var(key)
            .ok()
            .or_else(|| read_env_value("../../.env", key))
        {
            println!("cargo:rustc-env={key}={value}");
        }
    }
}

fn read_env_value(path: &str, key: &str) -> Option<String> {
    let path = Path::new(path);
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}
