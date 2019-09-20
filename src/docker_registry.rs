use indicatif::{ProgressBar, ProgressStyle};
use tokio::codec::{FramedRead, LinesCodec};
use tokio::{prelude::*, runtime::Runtime};
use tokio_net::process::Command;

use crate::dto::{BuildInstruction};
use serde::{Deserialize};

use log::{error};
use crate::login::UserSession;
use std::path::PathBuf;
use std::process::Stdio;

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct DockerCredentials {
    Username: String,
    Secret: String,
}

pub fn get_access_credentials(client: &reqwest::Client, session: &UserSession) -> Option<String> {
    let docker_credentials: DockerCredentials = match client.get("https://vault.openhabx.com/get/docker-access.json").bearer_auth(&session.access_token).send() {
        Ok(mut response) => {
            let response: Result<DockerCredentials, _> = response.json();
            if let Ok(response) = response {
                response
            } else {
                error!("Unexpected response!\n{:?}", response.err().unwrap());
                return None;
            }
        }
        Err(err) => {
            error!("Failed to contact https://vault.openhabx.com/get/docker-access.json!\n{:?}", err);
            return None;
        }
    };

    Some(docker_credentials.Username + ":" + &docker_credentials.Secret)
}

pub(crate) fn build_images(runtime:&Runtime, docker_credentials: &str, build_instructions: &mut Vec<BuildInstruction>,
                    input_file_name:&PathBuf) {
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}");

    let pb = ProgressBar::new(build_instructions.len() as u64);
    pb.set_style(spinner_style.clone());
    pb.set_prefix("[4/6]");

    for build_instruction in build_instructions {
        pb.set_message(&format!("Building {} - arch {}", &build_instruction.filename, &build_instruction.arch));

        let mut child = Command::new("podman")
            .arg("build")
            .arg("-t")
            .arg(&build_instruction.image_name)
            .arg("-f")
            .arg(&build_instruction.filename)
            .arg(format!("--creds={}", &docker_credentials))
            .current_dir(input_file_name.parent().unwrap())
            .stdout(Stdio::piped())
            .spawn().unwrap();

        let stdout = child.stdout().take().expect("no stdout");

        let mut reader = FramedRead::new(stdout, LinesCodec::new());
        let pb_output = pb.clone();
        runtime.spawn(async move {
            while let Some(line) = reader.next().await {
                pb_output.set_message(&line.unwrap());
            }
        });

        let result = runtime.block_on(child).expect("To block on podman until it finished");

        build_instruction.build = result.success();

        // Determine the size
        let size_output = std::process::Command::new("podman")
            .arg("image")
            .arg("inspect")
            .arg(&build_instruction.image_name)
            .arg("--format={{.Size}}")
            .output();
        if let Ok(size_output) = size_output {
            let size = String::from_utf8(size_output.stdout).unwrap();
            let size = size.trim();
            if let Ok(size) = size.parse() {
                build_instruction.image_size = size;
            }
        }

        pb.inc(1);
        if !build_instruction.build {
            error!("Failed to build {} - arch {}", build_instruction.filename, build_instruction.arch);
        }
    }
    pb.finish();
}

pub(crate) fn upload_images(runtime:&Runtime,docker_credentials: &str, build_instructions: &mut Vec<BuildInstruction>,
                     input_file_name:&PathBuf) {
    let spinner_style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{prefix:.bold.dim} {spinner} {wide_msg}");

    let pb = ProgressBar::new(build_instructions.len() as u64);
    pb.set_style(spinner_style.clone());
    pb.set_prefix("[5/6]");

    for build_instruction in build_instructions {
        if !build_instruction.build {
            pb.inc(1);
            continue;
        }
        pb.set_message(&format!("Upload Image {}", &build_instruction.image_name));
        let mut child = Command::new("podman")
            .arg("push")
            .arg(&build_instruction.image_name)
            .arg(format!("--creds={}", &docker_credentials))
            .current_dir(input_file_name.parent().unwrap())
            .stdout(Stdio::piped())
            .spawn().expect("starting podman for pushing images");

        let stdout = child.stdout().take().expect("no stdout");

        let pb_output = pb.clone();
        let mut reader = FramedRead::new(stdout, LinesCodec::new());
        runtime.spawn(async move {
            while let Some(line) = reader.next().await {
                pb_output.set_message(&line.unwrap());
            }
        });

        let result = runtime.block_on(child).expect("To block on podman until it finished");

        build_instruction.uploaded = result.success();
        pb.inc(1);
        if !build_instruction.uploaded {
            error!("Failed to push {}", build_instruction.image_name);
        }
    }

    pb.finish();
}