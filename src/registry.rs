use crate::dto::addons;
use std::fs::File;
use std::io::{Write, Read};
use std::time::{SystemTime, Duration};
use log::{error};

pub fn addon_registry(client: &reqwest::Client) -> Option<addons::AddonEntryMap> {
    let registry_cache = dirs::config_dir().unwrap().join(".ohx_registry_cache");
    let cache_time: Option<Duration> = registry_cache.metadata().and_then(|m| m.modified()).ok().and_then(|m| SystemTime::now().duration_since(m).ok());
    let mut registry_content: Option<addons::AddonEntryMap> = None;
    if let Some(duration) = cache_time {
        if duration.as_secs() < 500 {
            let mut buffer = Vec::new();
            if let Ok(mut f) = File::open(&registry_cache) {
                if let Ok(_) = f.read_to_end(&mut buffer) {
                    if let Ok(v) = serde_json::from_slice(&buffer) {
                        registry_content = Some(v)
                    }
                }
            }
        }
    }

    let registry_cache = match registry_content {
        Some(v) => v,
        None => {
            match addons::get_addons_registry(&client) {
                Ok(v) => {
                    // Write to cache
                    File::create(&registry_cache).unwrap().write_all(&serde_json::to_vec(&v).unwrap()).unwrap();
                    v
                }
                Err(e) => {
                    error!("Failed to update registry cache: {:?}", e);
                    return None;
                }
            }
        }
    };

    Some(registry_cache)
}