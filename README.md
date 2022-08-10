# WatchMate

[InfiniTime](https://github.com/InfiniTimeOrg/InfiniTime/) smart watch companion app, visually optimized for GNOME mobile and desktop.

![watchmate_2022-08-08](/uploads/9fafad857ab2cb6fffa2b9ab47d9a187/watchmate_2022-08-08.png)

## Install

### ArchLinux

WatchMate is available on AUR as [watchmate-git](https://aur.archlinux.org/packages/watchmate-git), so you can install it by manually downloading PKGBUILD or with your AUR helper of choice, for example:

```
paru -S watchmate-git
```

### Flathub

TODO

## Build

### Requirements

- GNU/Linux OS
- [Bluez](http://www.bluez.org/download/)
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
- [ ] Release checklist
    - [x] Icon
    - [ ] About dialog
    - [x] AppStream metainfo
- [ ] Packaging and distribution
    - [ ] Flatpak
    - [x] AUR
