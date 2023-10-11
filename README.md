# WatchMate

Companion app for [InfiniTime](https://github.com/InfiniTimeOrg/InfiniTime/)-powered [PineTime](https://www.pine64.org/pinetime/) smart watch.

Visually optimized for GNOME, adaptive for phone and desktop, Linux only.

![watchmate_v0.4.0](https://user-images.githubusercontent.com/4020369/216776553-59d2081e-9729-4997-8021-0882296621a4.png)

## Features

- Current time service.
- Data reading: battery level, heart rate, steps count, firmware version.
- OTA firmware and external resources updates. Both, from manually specified DFU/resources files, or automatically downloaded from [InfiniTime releases](https://github.com/InfiniTimeOrg/InfiniTime/releases) for selected version.
- Media-player control.
- Notifications forwarding.

## Install

WatchMate is available on [Flathub](https://flathub.org/apps/details/io.gitlab.azymohliad.WatchMate):

```
flatpak install io.gitlab.azymohliad.WatchMate
```

Or via the following community-maintained distro packages:

[![Packaging status](https://repology.org/badge/vertical-allrepos/watchmate.svg)](https://repology.org/project/watchmate/versions)

## Build

### Native

##### Prerequisites

- [GTK4](https://gtk-rs.org/gtk4-rs/stable/latest/book/installation_linux.html)
- [Libadwaita](https://gtk-rs.org/gtk4-rs/stable/latest/book/libadwaita.html#linux)
- [Rust](https://www.rust-lang.org/tools/install)

##### Build and Run

To compile and run the project, execute the following command from repo directory:

```
cargo run --release
```

### Flatpak

##### Prerequisites

- [flatpak](https://www.flatpak.org/setup/)
- [flatpak-builder](https://docs.flatpak.org/en/latest/flatpak-builder.html)

##### Install Dependencies

```
flatpak install org.gnome.Platform//43 org.gnome.Sdk//43 org.freedesktop.Sdk.Extension.rust-stable//22.08
```

##### Build

```
flatpak-builder --user target/flatpak flatpak/io.gitlab.azymohliad.WatchMate.yml
```

##### Run

```
flatpak-builder --run target/flatpak flatpak/io.gitlab.azymohliad.WatchMate.yml watchmate
```

##### Install

```
flatpak-builder --install target/flatpak flatpak/io.gitlab.azymohliad.WatchMate.yml
```

Here and above, `target/flatpak` is the build directory. It's a convenient default for Rust project (`target` is already in `.gitignore`), but can be anything else.

## Tech Stack and Thanks

WatchMate stands on the shoulders of the following giants:

- [Rust](https://www.rust-lang.org/) programming language.
- [Relm4](https://relm4.org/), [GTK4](https://gtk.org/) ([rs](https://gtk-rs.org/)) and [Libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/) ([rs](https://world.pages.gitlab.gnome.org/Rust/libadwaita-rs/)) for GUI.
- [BlueR](https://world.pages.gitlab.gnome.org/Rust/libadwaita-rs/) (an official [BlueZ](http://www.bluez.org/) Bindings for Rust) for the bluetooth stack.
- Awesome parts of Rust ecosystem, like [tokio](https://tokio.rs/), [serde](https://serde.rs/), [reqwest](https://github.com/seanmonstar/reqwest), [zbus](https://gitlab.freedesktop.org/dbus/zbus/), [anyhow](https://github.com/dtolnay/anyhow) and others (see [Cargo.toml](Cargo.toml) for the full list).

I'm deeply grateful to all people behind these technologies. WatchMate wouldn't be possible without them: first, it would be technically unliftable; second, even with the alternatives, I probably wouldn't enjoy it so much, and joy is vitally important for hobby projects like this one.
