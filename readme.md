# Addon Registry Commandline tool

<img alt="OHX CLI Logo" align="right" src="https://github.com/openhab-nodes/website/raw/master/static/img/openhab-cli.svg.png" />

[![Build Status](https://github.com/openhab-nodes/cloud-addon-registry-cli/workflows/Integration/badge.svg)](https://github.com/davidgraeff/firestore-db-and-auth-rs/actions)
[![](https://meritbadge.herokuapp.com/ohx-addon-publish)](https://crates.io/crates/ohx-addon-publish)
[![](https://img.shields.io/badge/license-MIT-blue.svg)](http://opensource.org/licenses/MIT)

> OHX is a modern smarthome solution, embracing technologies like software containers for language agnostic extensibility.
> Written in Rust with an extensive test suite, OHX is fast, efficient, secure and fun to work on.

This is the repository of the command line utility to publish Addons to the [OHX Addon Registry](https://openhabx.com/addons).

## How to get started

* Download the CLI on https://github.com/openhab-nodes/cloud-addon-registry-cli/releases
  or via command line `wget https://github.com/openhab-nodes/cloud-addon-registry-cli/releases/latest`
  or via Cargo `cargo install ohx-addon-publish`
* Install `podman`: https://podman.io/getting-started/installation.
  For Windows users also see [Windows Subsystem for Linux Installation Guide for Windows 10](https://docs.microsoft.com/en-us/windows/wsl/install-win10).

The tool does the following:

1. It validates your addon.yml Addon description file.
2. Checks your login status. If not logged in yet, you will be redirected to https://openhabx.com/auth where you can
   create an account / login and grant the CLI access to your account.
3. If the registry
   * [+] contains an Addon which matches with the addon-id of the current directory,
   * [-] but you are not the owner,
   the procedure will be aborted.
4. The CLI builds your Addon for the architectures x86-64 and armv7 (raspberry pi 2+3) and armv8 (raspberry pi 4)
   via the `Dockerfile`s found in the directory of the addon.yml file.
5. Uploads the container images to the docker.io container registry.
6. Updates your addon.yml file to point to the uploaded images.
7. Adds or updates your addon to the OHX Addon Registry.


## Cross compiling for c / c++

One way is to use qemu (via a software container) and let the entire toolchain run under the target architecture:
```
sudo docker run --rm --privileged multiarch/qemu-user-static --reset -p yes
```