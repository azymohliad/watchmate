cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: 'watchmate',
  console: true,
  install: true,
  install_dir: bindir,
  command: [
    find_program('sh'),
    meson.project_source_root() / 'watchmate' / 'build-cargo.sh',
    meson.project_source_root() / 'Cargo.toml',
    meson.project_build_root() / 'target',
    '@OUTPUT@',
    meson.project_build_root() / 'cargo-home',
  ]
)
