use crate::GenResult;
use crate::config::{APP_ID, PROFILE};
use crate::glib::clone;
use crate::objects::errors::IntoResult;
use crate::objects::properties::CustomRGB;
use adw::prelude::AdwDialogExt;
use adw::prelude::AlertDialogExt;
use adw::prelude::ComboRowExt;
use adw::subclass::prelude::AdwDialogImpl;
use gdk4::RGBA;
use gettextrs::*;
use gio::AppInfo;
use gio::glib::user_cache_dir;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::*;
use log::*;
use std::path::PathBuf;
use std::{env, fs, path};

use crate::IconicWindow;

mod imp {
    use std::cell::Cell;

    use crate::objects::properties::CustomRGB;

    use super::*;

    use adw::subclass::prelude::PreferencesDialogImpl;
    use gdk4::RGBA;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/nl/emphisia/icon/settings/settings.ui")]
    pub struct PreferencesDialog {
        #[template_child]
        pub current_botton: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub default_dnd: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub dnd_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub radio_button_top: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub radio_button_bottom: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub select_bottom_color: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub use_builtin_icons_button: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub use_external_icon_button: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub use_external_icon_expander: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub use_builtin_icons_expander: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub use_system_color: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub store_top_images: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub ignore_custom: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub strict_button: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub strict_regeneration: TemplateChild<gtk::Switch>,
        #[template_child]
        pub automatic_regeneration: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub primary_color_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub primary_folder_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub secondary_color_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub secondary_folder_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub select_default_bottom: TemplateChild<adw::ButtonRow>,
        #[template_child]
        pub meta_drop_switch: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub preferences_page: TemplateChild<adw::PreferencesPage>,
        #[template_child]
        pub enable_advanced: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub enable_overflow: TemplateChild<adw::SwitchRow>,
        pub settings: gio::Settings,
        pub initialized: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesDialog {
        const NAME: &'static str = "PreferencesDialog";
        type Type = super::PreferencesDialog;
        type ParentType = adw::PreferencesDialog;

        fn new() -> Self {
            Self {
                default_dnd: TemplateChild::default(),
                dnd_switch: TemplateChild::default(),
                radio_button_top: TemplateChild::default(),
                radio_button_bottom: TemplateChild::default(),
                current_botton: TemplateChild::default(),
                settings: gio::Settings::new(APP_ID),
                select_bottom_color: TemplateChild::default(),
                use_builtin_icons_button: TemplateChild::default(),
                use_external_icon_button: TemplateChild::default(),
                use_external_icon_expander: TemplateChild::default(),
                use_builtin_icons_expander: TemplateChild::default(),
                use_system_color: TemplateChild::default(),
                store_top_images: TemplateChild::default(),
                automatic_regeneration: TemplateChild::default(),
                primary_color_row: TemplateChild::default(),
                primary_folder_color: TemplateChild::default(),
                secondary_color_row: TemplateChild::default(),
                secondary_folder_color: TemplateChild::default(),
                meta_drop_switch: TemplateChild::default(),
                strict_regeneration: TemplateChild::default(),
                ignore_custom: TemplateChild::default(),
                select_default_bottom: TemplateChild::default(),
                preferences_page: TemplateChild::default(),
                enable_advanced: TemplateChild::default(),
                enable_overflow: TemplateChild::default(),
                strict_button: TemplateChild::default(),
                initialized: Cell::new(false),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_instance_callbacks();
            klass.install_action("win.select_folder_settings", None, move |win, _, _| {
                win.select_path_filechooser();
            });
            klass.install_action("app.reset_color_primary", None, move |win, _, _| {
                win.imp()
                    .primary_folder_color
                    .set_rgba(&RGBA::from_rgb(164, 202, 238));
            });
            klass.install_action("app.reset_color_secondary", None, move |win, _, _| {
                win.imp()
                    .secondary_folder_color
                    .set_rgba(&RGBA::from_rgb(67, 141, 230));
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PreferencesDialog {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }
            let win = self;
            self.enable_advanced.connect_active_notify(glib::clone!(
                #[weak]
                win,
                move |_| {
                    if win.initialized.get() {
                        scroll_to_bottom(&win.preferences_page);
                    }
                }
            ));
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }

    impl WidgetImpl for PreferencesDialog {}
    impl AdwDialogImpl for PreferencesDialog {}
    impl PreferencesDialogImpl for PreferencesDialog {}
}

fn scroll_to_bottom(preferences_page: &adw::PreferencesPage) {
    // Get the first child which should be the ScrolledWindow
    if let Some(scrolled_window) = preferences_page
        .first_child()
        .and_then(|child| child.downcast::<gtk::ScrolledWindow>().ok())
    {
        // Get the vertical adjustment
        let vadjustment = scrolled_window.vadjustment();
        debug!("page size: {}", vadjustment.page_size());
        debug!("Value: {}", vadjustment.value());
        // Scroll to the bottom
        // You might want to do this in an idle callback to ensure the layout is complete
        glib::idle_add_local_once(move || {
            vadjustment.set_value(vadjustment.upper() - vadjustment.page_size());
        });
    }
}

glib::wrapper! {
    pub struct PreferencesDialog(ObjectSubclass<imp::PreferencesDialog>)
    @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog,
    @implements
        gtk::Accessible,
        gtk::Buildable,
        gtk::ConstraintTarget,
        gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PreferencesDialog {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let win = glib::Object::new::<Self>();
        let imp = win.imp();
        win.imp()
            .dnd_switch
            .set_active(win.imp().settings.boolean("default-dnd-activated"));
        if imp.settings.string("selected-accent-color") == "None" {
            imp.use_system_color.set_active(true);
        }
        if imp.settings.boolean("manual-bottom-image-selection") {
            imp.use_external_icon_button.set_active(true);
        }
        if imp.settings.string("default-dnd-action") == "bottom" {
            imp.radio_button_bottom.set_active(true);
        }
        imp.select_bottom_color
            .set_selected(imp.settings.uint("selected-accent-color-index"));
        win.load_set_colors();
        win.set_path_title();
        win.disable_color_dropdown(true);
        win.setup_settings();
        win.show_color_options();
        imp.initialized.set(true);
        win
    }

    fn load_set_colors(&self) {
        let imp = self.imp();
        let current_primary = imp.settings.string("primary-folder-color");
        let current_secondary = imp.settings.string("secondary-folder-color");
        imp.primary_folder_color
            .set_rgba(&RGBA::from_hex(current_primary.to_string()));
        imp.secondary_folder_color
            .set_rgba(&RGBA::from_hex(current_secondary.to_string()));
    }

    fn setup_settings(&self) {
        let imp = self.imp();
        imp.settings
            .bind("store-top-in-cache", &*imp.store_top_images, "active")
            .build();
        imp.settings
            .bind(
                "automatic-regeneration",
                &*imp.automatic_regeneration,
                "active",
            )
            .build();
        imp.settings
            .bind("allow-meta-drop", &*imp.meta_drop_switch, "active")
            .build();
        imp.settings
            .bind("advanced-settings", &*imp.enable_advanced, "active")
            .build();
        imp.settings
            .bind("mask-enabled", &*imp.enable_overflow, "active")
            .build();
        imp.settings
            .bind("default-dnd-activated", &*imp.dnd_switch, "active")
            .build();
        imp.settings
            .bind("strict-regeneration", &*imp.strict_button, "active")
            .invert_boolean()
            .build();
        imp.settings
            .bind("strict-regeneration", &*imp.strict_regeneration, "active")
            .invert_boolean()
            .build();
        imp.settings
            .bind("ignore-custom", &*imp.ignore_custom, "active")
            .build();
        imp.settings
            .bind(
                "manual-bottom-image-selection",
                &*imp.use_external_icon_button,
                "active",
            )
            .build();
        imp.select_bottom_color.connect_selected_item_notify(clone!(
            #[weak (rename_to = this)]
            self,
            move |_| {
                this.get_selected_accent_color(false);
                this.show_color_options();
            }
        ));

        imp.radio_button_top.connect_toggled(clone!(
            #[weak (rename_to = this)]
            self,
            move |_| {
                this.dnd_radio_state();
            }
        ));
        imp.use_system_color.connect_active_notify(clone!(
            #[weak (rename_to = this)]
            self,
            move |_| {
                this.disable_color_dropdown(false);
                this.show_color_options();
            }
        ));
        imp.primary_folder_color.connect_rgba_notify(clone!(
            #[weak (rename_to = this)]
            self,
            move |_| {
                let imp = this.imp();
                let color = imp.primary_folder_color.rgba();
                let _ = imp
                    .settings
                    .set_string("primary-folder-color", &color.to_hex());
            }
        ));
        imp.secondary_folder_color.connect_rgba_notify(clone!(
            #[weak (rename_to = this)]
            self,
            move |_| {
                let imp = this.imp();
                let color = imp.secondary_folder_color.rgba();
                let _ = imp
                    .settings
                    .set_string("secondary-folder-color", &color.to_hex());
            }
        ));
    }

    fn disable_color_dropdown(&self, init: bool) {
        let imp = self.imp();
        let switch_state = imp.use_system_color.is_active();
        match switch_state {
            true => {
                if !init {
                    let _ = imp.settings.set("selected-accent-color", "None");
                }
            }
            false => {
                self.get_selected_accent_color(init);
            }
        };
    }

    fn get_selected_accent_color(&self, init: bool) {
        let color_vec = vec![
            "Blue", "Teal", "Green", "Yellow", "Orange", "Red", "Pink", "Purple", "Slate", "Custom",
        ];
        let imp = self.imp();
        let selected_index = imp.select_bottom_color.selected() as usize;
        let selected_color = color_vec[selected_index];
        debug!("Selected accent color: {selected_color}");
        if !init {
            let _ = imp.settings.set("selected-accent-color", selected_color);
            let _ = imp
                .settings
                .set("selected-accent-color-index", selected_index as u32);
        }
    }

    fn set_path_title(&self) {
        let imp = self.imp();
        let current_path = PathBuf::from(&imp.settings.string("folder-svg-path"));
        let path = || -> GenResult<String> {
            Ok(if let Some(stem) = current_path.file_stem() {
                stem.to_string_lossy().into_owned().to_string()
            } else {
                current_path.to_str().into_result()?.to_owned()
            })
        }()
        .unwrap_or("Unknown".to_string());
        imp.current_botton.set_property("subtitle", path);
    }

    fn select_path_filechooser(&self) {
        glib::spawn_future_local(glib::clone!(
            #[strong(rename_to=win)]
            self,
            async move {
                let filters = gio::ListStore::new::<gtk::FileFilter>();
                let filter = gtk::FileFilter::new();
                filter.add_mime_type("image/*");
                filters.append(&filter);
                let dialog = gtk::FileDialog::builder()
                    .title(gettext("Open Document"))
                    .modal(true)
                    .filters(&filters)
                    .build();
                let parent = win.parent().unwrap();
                debug!("Parent type: {}", parent.value_type());
                let file = dialog
                    .open_future(parent.downcast_ref::<IconicWindow>())
                    .await;

                match file {
                    Ok(x) => {
                        info!("{:#?}", &x.path().unwrap());
                        let path: &str = &x.path().unwrap().into_os_string().into_string().unwrap();
                        win.can_error(win.set_path(path));
                    }
                    Err(y) => {
                        warn!("{:#?}", y);
                    }
                }
            }
        ));
    }

    fn set_path(&self, path: &str) -> GenResult<()> {
        self.copy_folder_image_to_cache(path::PathBuf::from(path))?;
        self.imp().settings.set("folder-svg-path", path)?;
        self.set_path_title();
        Ok(())
    }

    fn copy_folder_image_to_cache(&self, original_path: path::PathBuf) -> GenResult<()> {
        let cache_dir = self.get_cache_path();
        let file_name = format!(
            "folder.{}",
            original_path.extension().unwrap().to_str().unwrap()
        );
        self.imp()
            .settings
            .set("folder-cache-name", file_name.clone())?;
        let cache_path = cache_dir.join(file_name);
        fs::copy(original_path, cache_path)?;
        Ok(())
    }

    fn can_error<T>(&self, result: GenResult<T>) {
        let _ = result.map_err(|e| {
            const RESPONSE_OK: &str = "OK";
            let dialog = adw::AlertDialog::builder()
                .heading(gettext("Error"))
                .body(&e.to_string())
                .default_response(RESPONSE_OK)
                .build();
            dialog.add_response(RESPONSE_OK, "ok");
            dialog.present(Some(self))
        });
    }

    pub fn dnd_radio_state(&self) {
        let imp = self.imp();
        let radio_button = imp.radio_button_top.is_active();
        debug!("Radio button changed: button 1 is {}", radio_button);
        match radio_button {
            true => {
                let _ = imp.settings.set("default-dnd-action", "top");
            }
            false => {
                let _ = imp.settings.set("default-dnd-action", "bottom");
            }
        }
    }

    pub fn get_cache_path(&self) -> PathBuf {
        let cache_path = match env::var("XDG_CACHE_HOME") {
            Ok(value) => PathBuf::from(value),
            Err(_) => {
                let config_dir = PathBuf::from(env::var("HOME").unwrap())
                    .join(".cache")
                    .join(format!("nl.emphisia.icon"));
                if !config_dir.exists() {
                    fs::create_dir(&config_dir).unwrap();
                }
                config_dir
            }
        };
        debug!("cache path {:?}", cache_path);
        cache_path
    }

    #[template_callback]
    pub async fn open_image_cache(&self, _button: adw::ButtonRow) {
        let file = gio::File::for_path(format!(
            "{}/top_images/",
            user_cache_dir().to_str().unwrap()
        ))
        .uri();
        if let Err(e) = AppInfo::launch_default_for_uri(&file, None::<&gio::AppLaunchContext>) {
            self.can_error::<()>(Err(std::boxed::Box::new(e)));
        };
    }
}
