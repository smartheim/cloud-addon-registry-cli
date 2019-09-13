#![deny(warnings)]

pub mod dto;

use structopt::StructOpt;
use std::path::PathBuf;
use std::fs::File;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use dto::addons;

use log::{info, debug, warn, error};
use env_logger::Env;
use std::time::SystemTime;
use failure::_core::time::Duration;

use console::{style, Emoji};
use indicatif::{ProgressBar, ProgressStyle};
use std::thread;

static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍  ", "");
static PAPER: Emoji<'_, '_> = Emoji("📃  ", "");
//static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");

const OAUTH_CLIENT_ID: &'static str = "addoncli";

#[derive(Debug, StructOpt)]
#[structopt(author, about)]
struct Opt {
    /// Verbose mode (-v, -vv, -vvv, etc.)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Build directory. All intermediate build artifacts including the generated software containers
    /// are stored in here. Just delete this directory to perform a clean build.
    #[structopt(short, long, parse(from_os_str), default_value = "out")]
    build_directory: PathBuf,

    /// The input addon description file.
    #[structopt(short, long, parse(from_os_str), default_value = "addons.yml")]
    input_file: PathBuf,

    /// Only validate the addons.yml file and exit
    #[structopt(long)]
    validate_only: bool,

    /// Only login, store the session token and exit
    #[structopt(long, short)]
    login_only: bool,

    /// Logout, remove the session token and exit
    #[structopt(long)]
    logout: bool,

    /// Your https://openhabx.com username / email address. This is only used if you are not logged in yet.
    #[structopt(long, short, env = "OHX_USERNAME")]
    username: Option<String>,

    /// Your https://openhabx.com password. This is only used if you are not logged in yet.
    /// Pass this via stdin or environment variable.
    #[structopt(env = "OHX_PASSWORD")]
    password: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct UserSession {
    pub refresh_token: Option<String>,
    pub access_token: String,
    // unix timestamp in seconds
    pub access_token_expires: i64,
    pub user_id: String,
    pub user_email: String,
    pub user_display_name: String,
}

#[derive(Serialize)]
struct TokenRequestForRefreshToken {
    refresh_token: String,
    client_id: String,
    grant_type: String,
}

#[derive(Serialize)]
struct TokenRequestForDevice {
    device_code: String,
    client_id: String,
    grant_type: String,
}

#[derive(Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    // "bearer"
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: String, // Space delimiter
}

#[derive(Deserialize)]
pub struct ErrorResult {
    pub error: String,
}

impl From<String> for ErrorResult {
    fn from(message: String) -> Self {
        serde_json::from_str(&message).unwrap()
    }
}


/// Users id, email, display name and a few more information
#[allow(non_snake_case)]
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct FirebaseAuthUser {
    pub localId: Option<String>,
    pub email: Option<String>,
    pub displayName: Option<String>,
}

/// Your user information query might return zero, one or more [`FirebaseAuthUser`] structures.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct FirebaseAuthUserResponse {
    pub users: Vec<FirebaseAuthUser>,
}


fn main() {
    let client = reqwest::Client::new();
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}");

    // Parse command line and setup logger
    let opt = Opt::from_args();
    let level = match opt.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug"
    };
    env_logger::from_env(Env::default().default_filter_or(level)).default_format_timestamp(false).init();
    debug!("{:?}", opt);


    // Read in yaml file and validate
    let input_file = opt.input_file.to_str().unwrap();
    info!("{} Validating input file {}", style("[2/4]").bold().dim(), input_file);
    let _input_file = match addons::open_validate_addons_file(input_file) {
        Ok(v) => v,
        Err(e) => {
            match e.downcast::<std::io::Error>() {
                Ok(_) => error!("{} Did not find the addon description file: {}!", LOOKING_GLASS, input_file),
                Err(e) => error!("Input file validation failed!\n{:?}", e)
            };
            return;
        }
    };

    if opt.validate_only {
        return;
    }

    // Read OHX session
    let mut buffer = Vec::new();
    let session: Option<UserSession> = match File::open(dirs::config_dir().unwrap().with_file_name(".ohx_login")) {
        Ok(mut f) => {
            match f.read_to_end(&mut buffer) {
                Ok(_) => {
                    match serde_json::from_slice(&buffer) {
                        Ok(v) => Some(v),
                        Err(_) => None
                    }
                }
                _ => None
            }
        }
        Err(_) => None
    };

    let session: Option<UserSession> = if let Some(session) = &session {
        info!("{} Getting access token", style("[2/4]").bold().dim());

        if let Some(refresh_token) = &session.refresh_token {
            let token_request = TokenRequestForRefreshToken {
                refresh_token: refresh_token.clone(),
                client_id: OAUTH_CLIENT_ID.to_string(),
                grant_type: "refresh_token".to_string(),
            };

            let r = client.post("oauth.openhabx.com/token").form(&token_request).send();
            if r.is_err() {
                error!("{} Failed to contact oauth.openhabx.com/token!\n{:?}", style("[2/4]").bold().dim(), r.err().unwrap());
                return;
            }
            let mut r = r.unwrap();
            if r.status() != 200 {
                warn!("Could not refresh access token. Login required");
                None
            } else {
                let r: OAuthTokenResponse = r.json().unwrap();
                Some(UserSession {
                    refresh_token: session.refresh_token.clone(),
                    access_token: r.access_token,
                    access_token_expires: chrono::Utc::now().timestamp() + r.expires_in - 10,
                    user_id: session.user_id.clone(),
                    user_email: session.user_email.clone(),
                    user_display_name: session.user_display_name.clone(),
                })
            }
        } else {
            None
        }
    } else {
        None
    };

    let session: UserSession = if session.is_some() {
        session.unwrap()
    } else {
        #[derive(Serialize)]
        struct AuthRequest {
            client_id: String,
            client_name: String,
            response_type: String,
            scope: String,
        };
        let token_request = AuthRequest {
            client_id: OAUTH_CLIENT_ID.to_string(),
            client_name: "OHX Addon Registry CLI".to_string(),
            response_type: "device".to_string(),
            scope: "offline_access addons profile".to_string(),
        };
        #[derive(Deserialize)]
        pub struct DeviceFlowResponse {
            pub device_code: String,
            pub user_code: String,
            pub verification_uri: String,
            pub interval: u32,
            pub expires_in: i64,
        }

        let r = client.post("oauth.openhabx.com/authorize").form(&token_request).send();
        if r.is_err() {
            error!("{} Failed to contact oauth.openhabx.com/authorize!\n{:?}", style("[2/4]").bold().dim(), r.err().unwrap());
            return;
        }
        let mut r = r.unwrap();
        if r.status() != 200 {
            let message = match r.status().as_u16() {
                400 => serde_json::from_str::<ErrorResult>(r.text().as_ref().unwrap()).unwrap().error,
                _ => r.text().unwrap()
            };
            error!("{} Could not start authorisation process: {}", style("[2/4]").bold().dim(), &message);
            return;
        }

        let device_flow_response: DeviceFlowResponse = r.json().unwrap();
        warn!("{} Please authorize the CLI to publish Addons on your behalf.\n\tURL: {}", style("[2/4]").bold().dim(), &device_flow_response.verification_uri);
        let _ = webbrowser::open(&device_flow_response.verification_uri);

        let expires_in = chrono::Utc::now().timestamp() + device_flow_response.expires_in;

        let pb = ProgressBar::new(device_flow_response.expires_in as u64);
        pb.set_style(spinner_style.clone());
        pb.set_prefix("[2/4]");

        let token_response: Option<OAuthTokenResponse> = loop {
            let diff = expires_in - chrono::Utc::now().timestamp();
            pb.set_message(&format!("Waiting for authorizsation! Request expires in {} s.", diff));
            pb.inc(2);
            thread::sleep(Duration::from_secs(2));

            let token_request = TokenRequestForDevice {
                device_code: device_flow_response.device_code.clone(),
                client_id: OAUTH_CLIENT_ID.to_string(),
                grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            };
            let response = client.post("oauth.openhabx.com/token").form(&token_request).send();
            if response.is_err() {
                error!("{} Failed to contact oauth.openhabx.com/token!\n{:?}", style("[2/4]").bold().dim(), response.err().unwrap());
                return;
            }
            let mut response = response.unwrap();
            if response.status() == 200 {
                let r: OAuthTokenResponse = r.json().unwrap();
                break (Some(r));
            }
            if response.status() == 400 {
                let response = ErrorResult::from(response.text().unwrap());
                if &response.error != "authorization_pending" {
                    error!("{} Server response: {}", style("[2/4]").bold().dim(), &response.error);
                    break (None);
                }
            }
            if diff < 0 {
                error!("Request expired");
                break (None);
            }
        };

        pb.finish_with_message("done!");

        if token_response.is_none() {
            return;
        }
        let token_response = token_response.unwrap();

        // get user information if possible
        let user_data: FirebaseAuthUser = match client.get("oauth.openhabx.com/userinfo").bearer_auth(&token_response.access_token).send() {
            Ok(mut response) => {
                let response: Result<FirebaseAuthUserResponse, _> = response.json();
                if let Ok(response) = response {
                    response.users.into_iter().next().unwrap()
                } else {
                    error!("{} Unexpected response userinfo response!\n{:?}", style("[2/4]").bold().dim(), response.err().unwrap());
                    return;
                }
            }
            Err(err) => {
                error!("{} Failed to contact oauth.openhabx.com/userinfo!\n{:?}", style("[2/4]").bold().dim(), err);
                return;
            }
        };

        UserSession {
            refresh_token: token_response.refresh_token.clone(),
            access_token: token_response.access_token,
            access_token_expires: chrono::Utc::now().timestamp() + token_response.expires_in - 10,
            user_id: user_data.localId.unwrap_or_default(),
            user_email: user_data.email.unwrap_or_default(),
            user_display_name: user_data.displayName.unwrap_or_default(),
        }
    };
//TODO oauth

    info!("You are logged in as {} ({})", session.user_email, &session.user_id);

    if opt.login_only {
        return;
    }

    info!("{} {} Updating registry index", style("[3/4]").bold().dim(), PAPER);
    let registry_cache = dirs::config_dir().unwrap().with_file_name(".ohx_registry_cache");
    let cache_time: Option<Duration> = registry_cache.metadata().and_then(|m| m.modified()).ok().and_then(|m| SystemTime::now().duration_since(m).ok());
    let registry_content: Option<addons::AddonEntryMap> = match cache_time {
        Some(duration) =>
            if duration.as_secs() < 500 {
                let mut buffer = Vec::new();
                match File::open(&registry_cache) {
                    Ok(mut f) => {
                        match f.read_to_end(&mut buffer) {
                            Ok(_) => {
                                match serde_json::from_slice(&buffer) {
                                    Ok(v) => Some(v),
                                    Err(_) => None
                                }
                            }
                            _ => None
                        }
                    }
                    Err(_) => None
                }
            } else {
                None
            },
        _ => None
    };

    let _registry_cache = match registry_content {
        Some(v) => v,
        None => {
            match addons::get_addons_registry(&client) {
                Ok(v) => {
// Write to cache
                    File::open(&registry_cache).unwrap().write_all(&serde_json::to_vec(&v).unwrap()).unwrap();
                    v
                }
                Err(e) => {
                    error!("Failed to update registry cache: {:?}", e);
                    return;
                }
            }
        }
    };

    // TODO Get docker access token
}