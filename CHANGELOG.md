# Changelog

## Unreleased

- Added persistent settings.
- Added an option to run in the background.
- Added an option to auto-start on login.
- Added automatic reconnection when the connection is lost.
- Reworked automatic connection on startup.
- Added an "About" dialog.
- Made minor UI improvements.


## v0.4.6 - 2023-10-10

- Fixed build with musl.


## v0.4.5 - 2023-05-19

- Added mobile-friendly declaration for Phosh.
- Bundled symbolic icons.


## v0.4.4 - 2023-03-19

- Added support for older versions of InfiniTime (tested with v0.8.3).


## v0.4.3 - 2023-03-15

- Warn when trying to flash unsupported firmware release or mismatching resource version.
- Disabled resource flash and download buttons for releases without resources.


## v0.4.2 - 2023-02-20

- Improved notifications permission error message.


## v0.4.1 - 2023-02-11

- Fixed minimum window width being affected by selected firmware version length.


## v0.4.0 - 2023-02-04

- Updated dependencies.
- Added basic desktop notifications propagation.
- Added external resources support.
- Added step count reading.
- Battery level now updates automatically, instead of at startup only.
- Implemented proper PineTime disconnection handling.
- Removed scanning toggle button, discovery is more automatic now.
- Various minor fixes and improvements.


## v0.3.0 - 2022-09-01

- Replaced println with a proper logging.
- Media players list now updates automatically and immediately.
- Added file save dialog for firmware download.
- Removed xdg-download filesystem permission for Flatpak.
- Enabled GPU acceleration for Flatpak.
- Fixed crash on startup if bluetooth adapter is disabled or missing.


## v0.2.0 - 2022-08-18

- Replaced file chooser widget with the dialog window.
- Removed unwanted toast notifications.
- Implemented media player integration.
- Various UI improvements.


## v0.1.0 - 2022-08-10

Initial release.
