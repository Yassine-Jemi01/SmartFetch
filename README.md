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

* **Blazing Fast:** Built with pure C and direct system APIs for minimal resource usage.
* **Dynamic ASCII Art:** Automatically detects your Linux distribution (Fedora, Arch, Debian, etc.) and renders the corresponding logo.
* **Terminal Color Palette:** Displays a 16-color block palette at the bottom.
* **Detailed System Insights:** Displays CPU information, temperatures, RAM usage and type, display resolution, disk space, GPU, package counts, and OS uptime where supported.
* **Update Check:** Uses the GitHub API over HTTPS to check the latest repository commit.

## Prerequisites

SmartFetch needs GCC, Make, and the libcurl development headers (used by the update checker). No Linux file capabilities are required. RAM type detection reads firmware DMI data when permissions allow it; otherwise it tries dmidecode when available or displays N/A.

~~~bash
# Debian / Ubuntu
sudo apt install build-essential libcurl4-openssl-dev

# Fedora
sudo dnf install gcc make libcurl-devel

# Arch
sudo pacman -S base-devel curl
~~~

## Build & Installation

~~~bash
git clone https://github.com/Yassine-Jemi01/SmartFetch.git
cd SmartFetch
make
make test
sudo make install
~~~

By default, installation places the executable in /usr/local/bin and ASCII art under /usr/local/share/smartfetch. Override PREFIX or DESTDIR for custom or staged installs.

**Security:** make install does not grant Linux capabilities or other elevated privileges. The program runs as the invoking user.

<p align="center">
  <img width="800" height="449" alt="sfetch" src="https://github.com/user-attachments/assets/62994abe-982b-47eb-8827-0029a0fad86b" />
</p>

## Usage

~~~bash
sfetch
sfetch --help
sfetch --update
~~~

## Windows Support (Beta)

Compile using GCC / MinGW from the project root:

~~~bash
gcc C_code/main.c C_code/ui.c C_code/collect_windows.c -I./H_code -o sfetch.exe -lcurl -ladvapi32
sfetch.exe
~~~

Windows builds require a MinGW-compatible libcurl development package. RAM type, CPU temperature, OS age, and Flatpak counts are not currently implemented on Windows and display N/A.

## Tests

~~~bash
make test
make sanitize
~~~

The smoke tests cover help output, invalid arguments, and a normal run. The sanitizer target rebuilds with AddressSanitizer and UndefinedBehaviorSanitizer.
