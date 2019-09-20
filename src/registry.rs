use crate::dto::{addons, BuildInstruction};
use std::fs::File;
use std::io::{Write, Read};
use std::time::{SystemTime, Duration};
use log::error;
use crate::dto::addons::AddonFileEntry;
use crate::login::UserSession;

pub(crate) fn addon_registry(client: &reqwest::Client) -> Option<addons::AddonEntryMap> {
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

pub(crate) fn post_to_registry(client: &reqwest::Client, build_instructions: &mut Vec<BuildInstruction>,
                        input_file: &AddonFileEntry,
                        session: &UserSession) -> bool {
    let mut reg_entry = addons::AddonFileEntryPlusStats {
        services: input_file.services.clone(),
        x_ohx_registry: input_file.x_ohx_registry.clone(),
        x_runtime: input_file.x_runtime.clone(),
        archs: build_instructions.iter().map(|e| e.arch.to_owned()).collect(),
        // Average of all arch sizes
        size: (build_instructions.iter().fold(0, |acc, build_instruction| acc + build_instruction.image_size) / build_instructions.len() as i64),
    };
    for (_service_id, service) in &mut reg_entry.services {
        // Only replace entries that have a "build" set
        if service.build.is_none() {
            continue;
        }
        service.build = None;
        service.image = Some(format!("docker.io/openhabx/{}:{}", &input_file.x_ohx_registry.id, &input_file.x_ohx_registry.version))
    }

    match client.post("https://registry.openhabx.com/addon").bearer_auth(&session.access_token).json(&reg_entry).send() {
        Ok(mut response) => {
            if response.status() != 200 {
                error!("Unexpected response!\n{:?}", response.text().unwrap());
                return false;
            }
        }
        Err(err) => {
            error!("Failed to contact https://vault.openhabx.com/get/docker-access.json!\n{:?}", err);
            return false;
        }
    };
    true
}