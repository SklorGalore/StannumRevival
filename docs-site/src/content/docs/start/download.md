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

Pre-releases such as `1.0.0-rc.1` come without the `.rpm`, as RPM versions
can't contain a hyphen. On Fedora, RHEL or openSUSE, run the AppImage.

The same page has the [Python package](/StannumRevival/guide/python/) and
`stannum-cli`, which [runs run files](/StannumRevival/guide/run-files/#running-without-the-app)
without the app:

| For | File | How to install |
| --- | --- | --- |
| Python 3.10 or newer, Windows | `stannum-<version>-cp310-abi3-win_amd64.whl` | `pip install stannum-<version>-cp310-abi3-win_amd64.whl` |
| Python 3.10 or newer, Mac with Apple silicon | `stannum-<version>-cp310-abi3-macosx_11_0_arm64.whl` | `pip install` the file |
| Python 3.10 or newer, Linux | `stannum-<version>-cp310-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl` | `pip install` the file |
| A terminal, Windows | `stannum-cli-<version>-windows-x64.zip` | Unzip it and put `stannum-cli.exe` on your `PATH`. |
| A terminal, Mac with Apple silicon | `stannum-cli-<version>-macos-arm64.tar.gz` | `tar -xzf` it and put `stannum-cli` on your `PATH`. |
| A terminal, Linux | `stannum-cli-<version>-linux-x64.tar.gz` | `tar -xzf` it and put `stannum-cli` on your `PATH`. |

What changed in each version is in the release notes on the same page, and in
[CHANGELOG.md](https://github.com/SklorGalore/StannumRevival/blob/main/CHANGELOG.md).

## Updates

Stannum checks for a newer release a few seconds after it starts. When there
is one, a bar under the toolbar says so, with the first lines of what's new:

- **Install and restart** downloads the update, installs it and opens the
  new version. If the open case has edits or a study that isn't saved,
  Stannum asks first, as it does before opening another case.
- **What's new** opens the release page.
- **Skip this version** stops the startup check offering that version.
  The next one is offered as usual.

Click the version number at the foot of the module rail, or choose **Check
for updates** in the command palette (**Ctrl+K**), to check at any time. A
dot beside the version means an update is waiting. To stop the startup
check, choose **Stop checking for updates at startup** in the palette.

Each update is signed, and Stannum only installs one whose signature matches
the project's key, which is built into the app. How each install updates:

| Installed from | The update |
| --- | --- |
| `-setup.exe` or `.msi` | Runs the new installer with a progress bar and no questions, then opens Stannum. |
| `.dmg` | Replaces Stannum in Applications. |
| `.AppImage` | Replaces the AppImage file. |
| `.deb` or `.rpm` | Installs the new package; your system asks for your password. |

The check downloads one small file (`latest.json`) from the latest GitHub
release. It never sends your cases or files anywhere.

## The first time you open it

The installers aren't signed by an identified developer yet, so your system
warns about them once:

- **macOS 15 (Sequoia) and later** says Apple could not verify Stannum is free
  of malware. Choose **Done**, then open **System Settings > Privacy &
  Security**, scroll to **Security**, choose **Open Anyway** next to Stannum,
  and confirm. After that it opens normally.
- **macOS 14 and earlier** says Apple can't check Stannum for malicious
  software. In Applications, right-click Stannum, choose **Open**, then
  **Open** again.
- If macOS says **"Stannum.app is damaged and can't be opened"** (versions
  0.4.1 and older), move the app to Applications and run this in Terminal,
  then open it again:

  ```sh
  xattr -dr com.apple.quarantine /Applications/Stannum.app
  ```
- **Windows** SmartScreen says "Windows protected your PC". Choose **More
  info**, then **Run anyway**.

Windows needs the Microsoft Edge WebView2 runtime, which Windows 10 and 11
already have. If it's missing, the installer downloads it.

## Other platforms

There is no installer for Intel Macs or for ARM Windows and Linux yet.
[Build from source](/StannumRevival/start/install/) on those.
