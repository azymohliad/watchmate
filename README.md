# WatchMate

InfiniTime companion app for GNOME desktop/mobile.

![watchmate_2022-08-05](https://user-images.githubusercontent.com/4020369/183119553-487882c8-8834-41ac-9e69-2b865f1081e9.png)

## Requirements

- GNU/Linux OS
- [Rust](https://www.rust-lang.org/tools/install)
- [GTK4](https://gtk-rs.org/gtk4-rs/git/book/installation_linux.html)

## Running

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
    - [ ] Icon
    - [ ] About dialog
    - [ ] Metadata
- [ ] Packaging and distribution
    - [ ] Flatpak
    - [ ] AUR
