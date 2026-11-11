project(
  'watchmate',
  'rust',
  version: '0.5.3',
  meson_version: '>= 0.59',
  license: 'GPL-3.0-or-later',
)

gnome = import('gnome')

dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.70')
dependency('gtk4', version: '>= 4.12.0')
dependency('libadwaita-1', version: '>= 1.5')

glib_compile_schemas = find_program('glib-compile-schemas', version: '>= 2.66', required: true)
cargo = find_program('cargo', version: '>= 1.75', required: true)

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

subdir('data')
subdir('watchmate')

gnome.post_install(
  gtk_update_icon_cache: true,
  glib_compile_schemas: true,
  update_desktop_database: true,
)
