
const OAUTH_CLIENT_ID: &'static str = "addoncli";
use serde::{Deserialize, Serialize};
use log::{info, warn, error};

#[derive(Deserialize, Serialize)]
pub struct UserSession {
    pub refresh_token: Option<String>,
    pub access_token: String,
    // unix timestamp in seconds
    pub access_token_expires: i64,
    pub user_id: String,
    pub user_email: String,
    pub user_display_name: String,
}

#[derive(Serialize)]
pub struct TokenRequestForRefreshToken {
    refresh_token: String,
    client_id: String,
    grant_type: String,
}

#[derive(Serialize)]
pub struct TokenRequestForDevice {
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
        serde_json::from_str(&message).expect("extracting json from a 400 error")
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


use std::fs::File;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{Read, Write};
use std::time::Duration;
use console::{style};
use std::thread;

pub fn perform_login(client: &reqwest::Client) -> Option<UserSession> {
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}");

    let user_session_file = dirs::config_dir().expect("config_dir to exist").join(".ohx_login");

    // Read OHX session
    let mut buffer = Vec::new();
    let session: Option<UserSession> = match File::open(&user_session_file) {
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
        if let Some(refresh_token) = &session.refresh_token {
            println!("{} Getting access token", style("[2/6]").bold().dim());
            let token_request = TokenRequestForRefreshToken {
                refresh_token: refresh_token.clone(),
                client_id: OAUTH_CLIENT_ID.to_string(),
                grant_type: "refresh_token".to_string(),
            };

            let r = client.post("https://oauth.openhabx.com/token").form(&token_request).send();
            if r.is_err() {
                error!("Failed to contact https://oauth.openhabx.com/token!\n{:?}", r.err().unwrap());
                return None;
            }
            let mut r = r.unwrap();
            match r.status().as_u16() {
                400 => {
                    warn!("Access token could not be refreshed. {}. Login required", &serde_json::from_str::<ErrorResult>(r.text().as_ref().unwrap()).unwrap().error);
                    None
                }
                200 => {
                    let r: OAuthTokenResponse = r.json().expect("an oauth token response from the /token endpoint");
                    Some(UserSession {
                        refresh_token: session.refresh_token.clone(),
                        access_token: r.access_token,
                        access_token_expires: chrono::Utc::now().timestamp() + r.expires_in - 10,
                        user_id: session.user_id.clone(),
                        user_email: session.user_email.clone(),
                        user_display_name: session.user_display_name.clone(),
                    })
                }
                v => {
                    error!("Unexpected response {} while refreshing access token: {}\nhttps://oauth.openhabx.com/token?auth={}",
                           v, &r.text().unwrap(), refresh_token);
                    return None;
                }
            }
        } else {
            info!("User session found, but no refresh token");
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

        let r = client.post("https://oauth.openhabx.com/authorize").form(&token_request).send();
        if r.is_err() {
            error!("Failed to contact https://oauth.openhabx.com/authorize!\n{:?}", r.err().unwrap());
            return None;
        }
        let mut r = r.unwrap();
        if r.status() != 200 {
            let message = match r.status().as_u16() {
                400 => serde_json::from_str::<ErrorResult>(r.text().as_ref().unwrap()).unwrap().error,
                _ => r.text().expect("a helping error message from /authorize")
            };
            error!("Could not start authorisation process: {}", &message);
            return None;
        }

        let device_flow_response: DeviceFlowResponse = r.json().expect("a device flow response from /authorize");
        warn!("Please authorize the CLI to publish Addons on your behalf.\n\tURL: {}", &device_flow_response.verification_uri);
        let _ = webbrowser::open(&device_flow_response.verification_uri);

        let expires_in = chrono::Utc::now().timestamp() + device_flow_response.expires_in;

        let diff = expires_in - chrono::Utc::now().timestamp();
        info!("Request expires in {} s.", diff);

        let pb = ProgressBar::new(device_flow_response.expires_in as u64);
        pb.set_style(spinner_style.clone());
        pb.set_prefix("[2/6]");

        let token_response: Option<OAuthTokenResponse> = loop {
            let diff = expires_in - chrono::Utc::now().timestamp();
            pb.set_message(&format!("Waiting for authorization! Request expires in {} s.", diff));
            pb.inc(2);
            pb.finish();
            thread::sleep(Duration::from_secs(2));

            let token_request = TokenRequestForDevice {
                device_code: device_flow_response.device_code.clone(),
                client_id: OAUTH_CLIENT_ID.to_string(),
                grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            };
            let response = client.post("https://oauth.openhabx.com/token").form(&token_request).send();
            if response.is_err() {
                error!("Failed to contact https://oauth.openhabx.com/token!\n{:?}", response.err().unwrap());
                return None;
            }
            let mut response = response.unwrap();
            match response.status().as_u16() {
                200 => {
                    let r = response.text().unwrap();
                    let r: OAuthTokenResponse = serde_json::from_str(&r).unwrap();
                    break (Some(r));
                }
                400 => {
                    let response = ErrorResult::from(response.text().unwrap());
                    if &response.error != "authorization_pending" {
                        error!("Server response: {}", &response.error);
                        break (None);
                    }
                }
                _ => {
                    error!("Server response: {}", &response.text().unwrap());
                    break (None);
                }
            };
            if diff < 0 {
                error!("Request expired");
                break (None);
            }
        };

        pb.finish_with_message("done!");

        if token_response.is_none() {
            return None;
        }
        let token_response = token_response.unwrap();

        // get user information if possible
        let user_data: FirebaseAuthUser = match client.get("https://oauth.openhabx.com/userinfo").bearer_auth(&token_response.access_token).send() {
            Ok(mut response) => {
                let response_text = response.text().unwrap();
                let response: Result<FirebaseAuthUser, _> = serde_json::from_str(&response_text);
                if let Ok(response) = response {
                    response
                } else {
                    error!("Unexpected response userinfo response!\n{:?}\nRaw: {}", response.err().unwrap(), response_text);
                    return None;
                }
            }
            Err(err) => {
                error!("Failed to contact https://oauth.openhabx.com/userinfo!\n{:?}", err);
                return None;
            }
        };

        let user_session = UserSession {
            refresh_token: token_response.refresh_token.clone(),
            access_token: token_response.access_token,
            access_token_expires: chrono::Utc::now().timestamp() + token_response.expires_in - 10,
            user_id: user_data.localId.unwrap_or_default(),
            user_email: user_data.email.unwrap_or_default(),
            user_display_name: user_data.displayName.unwrap_or_default(),
        };

        match File::create(&user_session_file) {
            Ok(mut f) => {
                let _ = f.write_all(&serde_json::to_vec(&user_session).expect("write user session to disk"));
            }
            Err(_) => {
                error!("Failed to write user session file at {}", user_session_file.to_str().unwrap());
                return None;
            }
        };

        user_session
    };

    Some(session)
}