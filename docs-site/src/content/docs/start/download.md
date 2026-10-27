---
title: Download
description: Install Stannum on Windows, macOS or Linux.
---

Installers for each release are on the
[GitHub releases page](https://github.com/SklorGalore/StannumRevival/releases/latest).
Pick the file for your computer from the **Assets** list:

| Computer | File | How to install |
| --- | --- | --- |
| Windows 10 or 11 | `Stannum_<version>_x64-setup.exe` | Run it. Installs for your user only; no administrator rights needed. |
| Windows, managed by IT | `Stannum_<version>_x64_en-US.msi` | Run it, or deploy it with your usual MSI tools. |
| Mac with Apple silicon (M1 or later) | `Stannum_<version>_aarch64.dmg` | Open it and drag Stannum into Applications. |
| Ubuntu, Debian | `Stannum_<version>_amd64.deb` | `sudo apt install ./Stannum_<version>_amd64.deb` |
| Fedora, RHEL, openSUSE | `Stannum-<version>-1.x86_64.rpm` | `sudo dnf install ./Stannum-<version>-1.x86_64.rpm` |
| Any other Linux | `Stannum_<version>_amd64.AppImage` | `chmod +x` it, then run it. |

What changed in each version is in the release notes on the same page, and in
[CHANGELOG.md](https://github.com/SklorGalore/StannumRevival/blob/main/CHANGELOG.md).

## The first time you open it

The installers aren't code-signed yet, so your system warns about them once:

- **macOS** says Apple can't check Stannum for malicious software. In
  Applications, right-click Stannum, choose **Open**, then **Open** again.
  After that it opens normally.
- **Windows** SmartScreen says "Windows protected your PC". Choose **More
  info**, then **Run anyway**.

Windows needs the Microsoft Edge WebView2 runtime, which Windows 10 and 11
already have. If it's missing, the installer downloads it.

## Other platforms

There is no installer for Intel Macs or for ARM Windows and Linux yet.
[Build from source](/StannumRevival/start/install/) on those.
