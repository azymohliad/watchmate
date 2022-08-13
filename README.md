# WatchMate

[InfiniTime](https://github.com/InfiniTimeOrg/InfiniTime/) smart watch companion app, visually optimized for GNOME mobile and desktop.

![collage_2022-08-13](/uploads/7c447fd1538be3b7364362d9eb09da03/collage_2022-08-13_readme.png)

## Install

### ArchLinux

WatchMate is available on AUR as [watchmate-git](https://aur.archlinux.org/packages/watchmate-git), so you can install it either [manually](https://wiki.archlinux.org/title/Arch_User_Repository#Installing_and_upgrading_packages) or with your [AUR helper](https://wiki.archlinux.org/title/AUR_helpers) of choice, for example:

```
paru -S watchmate-git
```

### Flathub

TODO

## Build

### Requirements

- GNU/Linux OS
- [Bluez](http://www.bluez.org/download/) (if you run mainstream GNU/Linux distro, you probably have it installed)
- [GTK4](https://gtk-rs.org/gtk4-rs/stable/latest/book/installation_linux.html)
- [Libadwaita](https://gtk-rs.org/gtk4-rs/stable/latest/book/libadwaita.html#linux)
- [Rust](https://www.rust-lang.org/tools/install)

### Build and run

To compile and run the project, execute the following command from repo directory:

```
cargo run --release
```

## Roadmap

- [x] Bluetooth device discovery, connecting to InfiniTime watch
- [x] Sharing time via Current Time Service
- [x] Reading data from the watch
    - [x] Battery level
    - [x] Firmware version
    - [x] Heart rate
- [x] OTA firmware update
    - [x] Firmware update from manually selected file
    - [x] Automatic firmware downloading from [InfiniTime releases](https://github.com/InfiniTimeOrg/InfiniTime/releases)
- [ ] Media-player control
- [ ] Secure pairing
- [ ] Notifications
- [ ] Settings
- [ ] About dialog
- [ ] Packaging and distribution
    - [ ] Flatpak
    - [x] AUR


## Tech stack and thanks

WatchMate stands on the shoulders of the following giants:

- [Rust](https://www.rust-lang.org/) programming language.
- [Relm4](https://relm4.org/), [GTK4](https://gtk.org/) ([rs](https://gtk-rs.org/)) and [Libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/) ([rs](https://world.pages.gitlab.gnome.org/Rust/libadwaita-rs/)) for GUI.
- [BlueR](https://world.pages.gitlab.gnome.org/Rust/libadwaita-rs/) (an official [BlueZ](http://www.bluez.org/) Bindings for Rust) for the bluetooth stack.
- Awesome parts of Rust ecosystem, like [tokio](https://tokio.rs/), [serde](https://serde.rs/), [reqwest](https://github.com/seanmonstar/reqwest), [zbus](https://gitlab.freedesktop.org/dbus/zbus/), [anyhow](https://github.com/dtolnay/anyhow) and others (see [Cargo.toml](Cargo.toml) for the full list). 

I'm really enjoying using all these technologies, and since joy is vitally important for hobby-projects like WatchMate, it wouldn't be possible without them. I'm deeply grateful to all people behind these techs.
