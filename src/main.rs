#![deny(warnings)]

pub mod dto;

use structopt::StructOpt;
use std::path::PathBuf;
use std::fs::File;
use std::io::Read;

use serde::Deserialize;
use dto::addons;

use log::{info, debug, error};
use env_logger::Env;

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
    #[structopt(short,long, parse(from_os_str), default_value = "addons.yml")]
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

#[derive(Deserialize)]
struct UserSession {
    pub refresh_token: String
}

fn main() {
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
    let _input_file = match addons::open_validate_addons_file(input_file) {
        Ok(v) => v,
        Err(e) => {
            match e.downcast::<std::io::Error>() {
                Ok(_) => error!("Did not find the addon description file: {}!", input_file),
                Err(e) => error!("Input file validation failed!\n{:?}", e)
            };
            return;
        }
    };

    debug!("{} validated", input_file);

    if opt.validate_only {
        return;
    }

    // Read OHX session
    let mut buffer = Vec::new();
    let session: Option<UserSession> = match File::open(dirs::config_dir().unwrap().join(".ohx_login")) {
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

    if let Some(_session) = &session {
        info!("Getting access token");
    }

    if session.is_none() {
        info!("You are not logged in");
    }

    if opt.login_only {
        return;
    }

}