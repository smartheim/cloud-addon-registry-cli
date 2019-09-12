use std::collections::{BTreeMap, HashMap};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

pub const REGISTRY_DATA_URL : &'static str ="https://raw.githubusercontent.com/openhab-nodes/addons-registry/master/extensions.json";
pub const REGISTRY_METADATA_URL : &'static str ="https://raw.githubusercontent.com/openhab-nodes/addons-registry/master/extensions_stats.json";

pub fn get_addons_registry(client: &reqwest::Client) -> Result<AddonEntryMap, failure::Error> {
    Ok(client.get(REGISTRY_DATA_URL).send()?.json()?)
}

pub fn get_addons_registry_metadata(client: &reqwest::Client) -> Result<AddonMapStats, failure::Error> {
    let t = client.get(REGISTRY_METADATA_URL).send()?.text()?;
    Ok(serde_json::from_str(&t)?)
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonPermission {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub standalone: bool,
}

pub type AddonPermissions = BTreeMap<String, AddonPermission>;

pub type AddonMapStats = BTreeMap<String, AddonStats>;

#[derive(Serialize, Deserialize)]
pub struct AddonStats {
    // voters
    pub v: u64,
    // rating points, sum
    pub p: i64,
    // downloads
    pub d: i64,
    // starts
    pub s: u64,
    // issues
    pub iss: u64,
    // last time checked
    pub t: i64,
}

pub type AddonEntryMap = BTreeMap<String, AddonRegistryEntry>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonEntryCommon {
    // Descriptive
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<HashMap<String, String>>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptions: Option<HashMap<String, String>>,
    pub authors: Vec<String>,
    pub manufacturers: Vec<String>,
    pub products: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    pub license: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog_url: Option<String>,
    #[serde(rename = "type")]
    pub type_field: String,

    // Identification
    pub id: String,
    pub version: String,
    pub status: Status,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonRegistryEntry {
    #[serde(flatten)]
    pub entry: AddonEntryCommon,
    pub owner: String,
    pub last_updated: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonFileEntry {
    pub services: HashMap<String, AddonService>,
    #[serde(rename = "x-ohx-registry")]
    pub x_ohx_registry: AddonEntryCommon,
    #[serde(rename = "x-runtime")]
    pub x_runtime: AddonRuntimeRequirements,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonFileEntryPlusStats {
    pub services: HashMap<String, AddonService>,
    #[serde(rename = "x-ohx-registry")]
    pub x_ohx_registry: AddonEntryCommon,
    #[serde(rename = "x-runtime")]
    pub x_runtime: AddonRuntimeRequirements,

    pub archs: Vec<String>,
    pub size: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonDetailedInfo {
    // Registry
    #[serde(default)]
    pub reviewed_by: Vec<String>,
    pub archs: Vec<String>,
    pub size: i64,

    // Runtime
    #[serde(flatten)]
    pub runtime: AddonRuntimeRequirements,
    pub services: HashMap<String, AddonService>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonService {
    // Security
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_allow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_add: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap_drop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devices: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildContext {
    pub context: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddonRuntimeRequirements {
    pub memory_min: i64,
    pub memory_max: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Permissions {
    pub mandatory: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub code: StatusCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descriptions: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StatusCode {
    AVAILABLE,
    REPLACED,
    REMOVED,
    UNMAINTAINED,
}

impl Default for StatusCode {
    fn default() -> Self {
        StatusCode::AVAILABLE
    }
}

pub fn open_validate_addons_file(filename: &str) -> Result<AddonFileEntry, failure::Error> {
    let addon_permissions: AddonPermissions = serde_json::from_str(include_str!("../../addon-permissions.json"))?;

    let mut f = File::open(filename)?;
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)?;
    let data: AddonFileEntry = serde_yaml::from_slice(buffer.as_slice())?;
    if data.services.is_empty() {
        return Err(failure::err_msg("No services defined"));
    }

    use regex::Regex;
    let pattern_registry = Regex::new(r"^[^:]*([:]\d+)?$").unwrap();
    let pattern_image_name = Regex::new(r"^[_\-a-z0-9]+(:[a-z0-9]+)?$").unwrap();

    for (service_id, service) in &data.services {
        if let Some(service_image) = &service.image {
            // Check image name
            let parts: Vec<&str> = service_image.split("/").collect();
            let image_name = if parts.len() == 2 {
                let registry_address = parts.get(0).unwrap();
                if !pattern_registry.is_match(registry_address) {
                    return Err(failure::err_msg(format!("Service registry address invalid for {}: {}", service_id, &service_image)));
                }
                parts.get(1).unwrap()
            } else {
                parts.get(0).unwrap()
            };
            if !pattern_image_name.is_match(image_name) {
                return Err(failure::err_msg(format!("Service image name invalid for {}: {}", service_id, image_name)));
            }
        }

        // Permissions
        if let Some(permissions) = &service.permissions {
            for permission in &permissions.mandatory {
                if !addon_permissions.contains_key(permission) {
                    return Err(failure::err_msg(format!("Mandatory permission unknown for {}: {}", service_id, &permission)));
                }
            }
            for permission in &permissions.optional {
                if !addon_permissions.contains_key(permission) {
                    return Err(failure::err_msg(format!("Optional permission unknown for {}: {}", service_id, &permission)));
                }
            }
        }

        // Ports
        if let Some(ports) = &service.ports {
            for port in ports {
                // Check for protocol "6060:6060/udp"
                let parts: Vec<&str> = port.split("/").collect();
                let port = if let Some(protocol) = parts.get(1) {
                    if *protocol != "udp" && *protocol != "tcp" {
                        return Err(failure::err_msg(format!("Ports pattern invalid. The part after / must be tcp or udp for {}: {}", service_id, &port)));
                    }
                    *protocol
                } else {
                    port
                };
                // Check for mapping "5000-5010:5000-5010"
                let parts: Vec<&str> = port.split(":").collect();
                if parts.len() > 2 {
                    return Err(failure::err_msg(format!("Ports pattern invalid. Maximum of two colon separated segments allowed for {}: {}", service_id, &port)));
                }
                // Check for ranges "5000-5010"
                for ports_maybe_range in parts {
                    let ports_maybe_range_segments: Vec<&str> = ports_maybe_range.split("-").collect();
                    if ports_maybe_range_segments.len() > 2 {
                        return Err(failure::err_msg(format!("Ports pattern invalid. A range can have only two segments {}: {}", service_id, &ports_maybe_range)));
                    }
                    let mut in_host_range = false;
                    // Check if port is in range and if the user didn't try to map to a port below 1024
                    for a_port in ports_maybe_range_segments {
                        match a_port.parse::<u16>() {
                            Ok(v) => {
                                if in_host_range && v < 1024 {
                                    return Err(failure::err_msg(format!("You cannot map to a port below 1024. Those are for privileged services only! For {}: {}", service_id, &a_port)));
                                }
                            }
                            Err(_) => return Err(failure::err_msg(format!("A port must be a number! For {}: {}", service_id, &a_port)))
                        };
                        in_host_range = true;
                    }
                }
            }
        }

        // Depends on
        if let Some(depends_on) = &service.depends_on {
            for depends in depends_on {
                if !data.services.contains_key(depends) {
                    return Err(failure::err_msg(format!("For now you can only depend on services defined in your own addon.yml. For {}: Did not find '{}'!", service_id, &depends)));
                }
            }
        }

        if let Some(volumes) = &service.volumes {
            for volume in volumes {
                let parts: Vec<&str> = volume.split(":").collect();
                if *parts.get(0).unwrap() != "logvolume" {
                    return Err(failure::err_msg(format!("There is currently only 'logvolume' supported. For {}. You requested volume: '{}'!", service_id, &volume)));
                }
            }
        }
    }
    Ok(data)
}

#[test]
fn open_validate_addons_file_test() {
    let d = open_validate_addons_file("tests/addon.yml").unwrap();
    assert_eq!(d.x_ohx_registry.id, "ohx-ci-test-addon");
}