# Changelog

## v0.4.3 - 2023-02-26

- Warn when trying to flash unsupported firmware release or mismatching resource version
- Disable resource flash and download buttons for releases without resources

## v0.4.2 - 2023-02-20

- Improve notifications permission error message

## v0.4.1 - 2023-02-11

- Fix minimum window width being affected by selected firmware version length


## v0.4.0 - 2023-02-04

- Updated dependencies
- Added basic desktop notifications propagation
- Added external resources support
- Added step count reading
- Battery level now updates automatically, instead of at startup only
- Implemented proper PineTime disconnection handling
- Removed scanning toggle button, discovery is more automatic now
- Various minor fixes and improvements


## v0.3.0 - 2022-09-01

- Replaced println with a proper logging
- Media players list now updates automatically and immediately
- Added file save dialog for firmware download
- Removed xdg-download filesystem permission for Flatpak
- Enabled GPU acceleration for Flatpak
- Fixed crash on startup if bluetooth adapter is disabled or missing


## v0.2.0 - 2022-08-18

- Replaced file chooser widget with the dialog window
- Removed unwanted toast notifications
- Implemented media player integration
- Various UI improvements


## v0.1.0 - 2022-08-10

Initial release
