# Addon Registry Commandline tool

This is the repository of the Addon registry command line utility to publish Addons to the OHX Addon Registry.
It works on Linux, MacOS and in a *Windows Subsystem for Linux* environment on Microsoft Windows
(See [Windows Subsystem for Linux Installation Guide for Windows 10](https://docs.microsoft.com/en-us/windows/wsl/install-win10)).

## How to get started

* Download the CLI on https://github.com/openhab-nodes/cloud-addon-registry-cli/releases
  or via command line `wget https://github.com/openhab-nodes/cloud-addon-registry-cli/releases/latest`
  or via Cargo `cargo install ohx-addon-publish`
* Install `podman`. See https://podman.io/getting-started/installation.

The tool does the following:

1. It checks your addon.yml syntax and if all required fields are set.
2. Updates the registry cache
3. Checks your login status. If not logged in yet, opens the addon registry page to register/login and to allow the CLI access to your account.
4. If the registry
   * [+] contains an Addon which matches with the addon-id of the current directory,
   * [-] but you are not the owner,
   the procedure will be aborted.
5. Build your Addon for the architectures x86-64 and armv7 (raspberry pi 2+3) and armv8 (raspberry pi 4)
   via the provided `Dockerfile`s.
6. Get a docker.io access token for the "openhabx" namespace.
7. Uploads the container images to the docker.io container registry.
8. Updates your addon.yml file to point to the uploaded images and if other referenced images can be found.
   If not all referenced images can be found the procedure will be aborted.
9. Adds your new addon version to the OHX Addon Registry.
