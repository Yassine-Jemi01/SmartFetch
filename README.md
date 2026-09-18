<h1 align="center">SmartFetch</h1>
<p align="center">
  <img
    src="https://github.com/user-attachments/assets/2cc55b49-ffb5-4ddc-9e70-eefc43bf9135"
    alt="SmartFetch Logo"
    width="300">
</p>

<p align="center">
  A lightweight, blazing-fast system information tool written in pure C.
</p>

<p align="center">
  <img src="https://img.shields.io/github/license/Yassine-Jemi01/SmartFetch?style=for-the-badge" alt="License">
  <img src="https://img.shields.io/github/stars/Yassine-Jemi01/SmartFetch?style=for-the-badge" alt="Stars">
  <img src="https://img.shields.io/github/forks/Yassine-Jemi01/SmartFetch?style=for-the-badge" alt="Forks">
  <img src="https://img.shields.io/github/issues/Yassine-Jemi01/SmartFetch?style=for-the-badge" alt="Issues">
</p>

---

<p align="center">
  <img width="1366" height="768" alt="Preview_Image" src="https://github.com/user-attachments/assets/31cc2325-4f74-4ec2-952c-351b5c446828" />
</p>

## Features

* **Blazing Fast:** Built with pure C and Direct System APIs for minimal resource usage and maximum performance.
* **Dynamic ASCII Art:** Automatically detects your Linux distribution (`Fedora`, `Arch`, `Debian`, etc.) and renders the corresponding logo.
* **Terminal Color Palette:** Displays a colorful 16-color block palette at the bottom for aesthetic screenshots.
* **Detailed System Insights:** Displays CPU specs, real-time temperatures, RAM type & usage (Linux, requires root or `cap_dac_read_search` — granted automatically by `make install`), display resolution, disk space, GPU, and OS uptime.
* **Standard Build System:** Uses a clean `Makefile` for simple, scalable compilation across platforms.

---

## Prerequisites

SmartFetch needs `gcc`/`make` and the **libcurl development headers** (used by the `--update` checker) to build. `libcap` is optional but recommended — without it, SmartFetch can still read the RAM type when run with `sudo`, but not as a regular user.

```bash
# Debian / Ubuntu
sudo apt install build-essential libcurl4-openssl-dev libcap2-bin

# Fedora
sudo dnf install gcc make libcurl-devel libcap

# Arch
sudo pacman -S base-devel curl libcap
```

## Build & Installation

Clone the repository and compile using `make`:

```bash
git clone https://github.com/Yassine-Jemi01/SmartFetch.git
cd SmartFetch
make
sudo make install
```

During `sudo make install`, SmartFetch also grants itself the `cap_dac_read_search` capability (if `setcap` is available) so it can read your RAM type from `/sys/firmware/dmi/tables/DMI` without needing `sudo` every time you run it afterward.

<p align="center">
  <img width="800" height="449" alt="sfetch" src="https://github.com/user-attachments/assets/62994abe-982b-47eb-8827-0029a0fad86b" />
</p>

## Windows Support (Beta)

Compile using GCC / MinGW from the project root:

```bash
gcc C_code/main.c C_code/ui.c C_code/collect_windows.c -I./H_code -o sfetch.exe -lcurl -ladvapi32
sfetch.exe
```

> **Note:** Windows builds require a MinGW-compatible libcurl development package. `RAM Type`, `CPU Temp`, `OS Age`, and `Flatpak` are not currently implemented on Windows and will show `N/A`.
