#![deny(warnings)]

pub mod dto;
mod login;
mod registry;
mod docker_registry;

use structopt::StructOpt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use dto::{addons,BuildInstruction};

use log::{info, debug, warn, error};
use env_logger::Env;

use console::{style, Emoji};
use std::str::FromStr;

pub static LOOKING_GLASS: Emoji<'_, '_> = Emoji("🔍  ", "");
pub static PAPER: Emoji<'_, '_> = Emoji("📃  ", "");
pub static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", ":-)");

// as of https://github.com/containerd/containerd/blob/master/platforms/platforms.go#L88
const ALLOWED_ARCHITECTURES: [&str; 4] = ["aarch64", "armhf", "i386", "amd64"];

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

fn main() {
    let client = reqwest::Client::new();

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
    let input_file_name: PathBuf = opt.input_file;
    let input_file_name_str = input_file_name.to_str().unwrap();
    println!("{} Validating input file {}", style("[1/6]").bold().dim(), input_file_name_str);
    let input_file = match addons::open_validate_addons_file(input_file_name_str) {
        Ok(v) => v,
        Err(e) => {
            match e.downcast::<std::io::Error>() {
                Ok(_) => error!("{} Did not find the addon description file: {}!", LOOKING_GLASS, input_file_name_str),
                Err(e) => error!("Input file validation failed!\n{:?}", e)
            };
            return;
        }
    };

    let mut build_instructions: Vec<BuildInstruction> = Vec::new();

    for entry in input_file_name.parent().unwrap().read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let filename = entry.file_name().into_string().unwrap();
            if filename.starts_with("Dockerfile") {
                let arch = if &filename == "Dockerfile" {
                    "amd64"
                } else {
                    &filename[11..]
                };
                if !ALLOWED_ARCHITECTURES.contains(&arch) {
                    warn!("A Dockerfile architecture is not supported: {}", arch);
                } else {
                    build_instructions.push(BuildInstruction {
                        arch: arch.to_owned(),
                        image_name: format!("docker.io/openhabx/{}_{}:{}", &input_file.x_ohx_registry.id, arch, &input_file.x_ohx_registry.version),
                        filename,
                        build: false,
                        uploaded: false,
                        image_size: 0,
                    });
                }
            }
        }
    }

    if build_instructions.len() == 0 {
        error!("No Dockerfiles found in {}. Cannot build Addon.\nPlease check the documentation or clone one the scaffolding repositories for working examples.",
               input_file_name.parent().unwrap().to_str().unwrap());
        return;
    }

    if opt.validate_only {
        return;
    }

    let session = login::perform_login(&client);
    if session.is_none() {
        return;
    }
    let session = session.unwrap();
    info!("You are logged in as {} ({})", session.user_email, &session.user_id);

    if opt.login_only {
        return;
    }

    println!("{} {} Updating registry index", style("[3/6]").bold().dim(), PAPER);
    let registry = registry::addon_registry(&client);
    if registry.is_none() {
        return;
    }
    let _registry = registry.unwrap();

    // Check for docker file
    // Check for podman executable
    println!("{} Checking podman", style("[3/6]").bold().dim());

    #[derive(Serialize, Deserialize)]
    struct PodmanVersionResult {
        #[serde(rename = "Version")]
        version: String
    }

    let version: Result<PodmanVersionResult, _> = std::process::Command::new("podman")
        .arg("version")
        .arg("--format")
        .arg("json")
        .output()
        .and_then(|f| serde_json::from_slice(&f.stdout).map_err(|o| std::io::Error::from(o)));

    if let Err(version) = version {
        error!("'podman' is required to build software containers. Please check https://podman.io/getting-started/installation. {:?}", version);
        return;
    }

    let podman_version = semver::Version::from_str(&version.unwrap().version).unwrap();

    if podman_version < semver::Version::new(1, 5, 0) {
        error!("'podman' 1.5.0 or better is required. Please check https://podman.io/getting-started/installation.");
    } else {
        info!("Found Podman version {}", podman_version);
    }

    // Get docker access credentials
    let docker_creds = docker_registry::get_access_credentials(&client,&session);
    if docker_creds.is_none() {
        return;
    }
    let docker_creds = docker_creds.unwrap();
    let runtime = tokio::runtime::Runtime::new().expect("Unable to start the runtime");

    docker_registry::build_images(&runtime,&docker_creds, &mut build_instructions,&input_file_name);
    docker_registry::upload_images(&runtime,&docker_creds, &mut build_instructions,
                                   &input_file_name);

    println!("{} Upload to registry", style("[6/6]").bold().dim());
    if !registry::post_to_registry(&client, &mut build_instructions,&input_file,&session) {
        return;
    }

    // Print summary
    println!("\nSummary for {} - Version {}\n", &input_file.x_ohx_registry.title, &input_file.x_ohx_registry.version);
    use prettytable::{Table, Row, Cell, cell};
    let mut table = Table::new();

    // Add a row per time
    table.add_row(prettytable::row!["Architecture", "Build", "Upload"]);
    for build_instruction in &build_instructions {
        table.add_row(Row::new(vec![
            Cell::new(&build_instruction.arch),
            match build_instruction.build {
                true => Cell::new("true").style_spec("bFg"),
                false => Cell::new("false").style_spec("BriH2")
            },
            match build_instruction.uploaded {
                true => Cell::new("true").style_spec("bFg"),
                false => Cell::new("false").style_spec("BriH2")
            }]));
    }
    table.printstd();
}