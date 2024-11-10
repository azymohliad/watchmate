fn main() {
    relm4_icons_build::bundle_icons(
        // Name of the file that will be generated at `OUT_DIR`
        "icon_names.rs",
        // Optional app ID
        Some("io.gitlab.azymohliad.WatchMate"),
        // Custom base resource path:
        None::<&str>,
        // Directory with custom icons (if any)
        None::<&str>,
        // List of icons to include
        [
            "arrow3-up",
            "bluetooth",
            "cross",
            "heart-filled",
            "heart-outline-thin",
            "refresh",
        ],
    );
}
