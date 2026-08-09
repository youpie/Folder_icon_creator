project(
  'Iconic',
  'rust',
  version: '2026.8.1',
  meson_version: '>= 0.62.0',
  default_options: [
    'warning_level=2',
    'werror=false',
  ],
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'nl.emphisia.icon'
prefix = get_option('prefix')
datadir = prefix / get_option('datadir')
iconsdir = datadir / 'icons'
gettext_package = meson.project_name()
podir = meson.project_source_root() / 'po'
pkgdatadir = datadir / meson.project_name()

if get_option('profile') == 'development'
  profile = 'Devel'
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD').stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
  application_id = '@0@.@1@'.format(base_id, profile)
else
  profile = ''
  version_suffix = ''
  application_id = base_id
endif

subdir('data')
subdir('src')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)