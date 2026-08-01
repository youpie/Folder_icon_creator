/* window.rs
 *
 * Copyright 2024 Youpie
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use crate::config::{APP_ICON, APP_ID, PROFILE};
use crate::glib::clone;
use crate::objects::errors::show_error_popup;
use crate::objects::file::file::File;
use crate::objects::properties::{BottomImageType, CustomRGB};
use adw::{prelude::*, subclass::prelude::*};
use gdk4::MemoryTexture;
use gettextrs::gettext;
use gio::glib::user_cache_dir;
use gio::prelude::SettingsExt;
use gtk::gdk::RGBA;
use gtk::{gdk, glib};
use image::*;
use log::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::RandomState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const DEFAULT_X_SLIDER: f64 = 0.0;
const DEFAULT_Y_SLIDER: f64 = 9.447;
const DEFAULT_SIZE_SLIDER: f64 = 24.0;

pub mod imp {
    use std::{cell::Cell, collections::HashMap, rc::Rc};

    use gio::MenuModel;

    use crate::{
        objects::properties::FileProperties,
        windows::{
            actions::{set_up_klass_actions, set_up_stateful_actions},
            drag_drop::setup_drag_drop_logic,
            drag_overlay::DragOverlay,
            preview_window::PreviewWindow,
        },
    };

    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/nl/emphisia/icon/window.ui")]
    pub struct IconicWindow {
        // Template widgets
        // Every item in this list, is something defined in
        // a blueprint file, if you want to control it from the code
        // it needs to be added here
        #[template_child]
        pub toolbar: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub open_top_icon: TemplateChild<gtk::Button>,
        #[template_child]
        pub image_view: TemplateChild<PreviewWindow>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub x_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub y_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub size: TemplateChild<gtk::Scale>,
        #[template_child]
        pub scale_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub image_loading_spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub monochrome_action_row: TemplateChild<adw::ExpanderRow>,
        #[template_child]
        pub monochrome_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        pub threshold_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub monochrome_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub reset_color: TemplateChild<gtk::Button>,
        #[template_child]
        pub monochrome_invert: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub main_status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub image_preferences: TemplateChild<adw::Clamp>,
        #[template_child]
        pub popover_menu: TemplateChild<gtk::PopoverMenu>,
        #[template_child]
        pub gesture_click: TemplateChild<gtk::GestureClick>,
        #[template_child]
        pub regeneration_osd: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub regeneration_osd_second: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub regeneration_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub drag_overlay: TemplateChild<DragOverlay>,
        #[template_child]
        pub image_menu_simple: TemplateChild<MenuModel>,
        #[template_child]
        pub image_menu: TemplateChild<MenuModel>,

        pub bottom_image_file: Arc<Mutex<Option<File>>>,
        pub top_image_file: Arc<Mutex<Option<File>>>,
        pub custom_mask: RefCell<Option<DynamicImage>>,

        pub default_colors: RefCell<HashMap<String, gdk::RGBA, RandomState>>,

        pub saved_file: Arc<Mutex<Option<gio::File>>>,
        pub file_created: Cell<bool>,
        pub image_saved: Cell<bool>,

        // This is used to, during the double image render of a drag operation, rename the larger file the same as the smaller file
        pub last_drag_n_drop_generated_name: RefCell<Option<gio::File>>,

        // This generated image is used to quickly add an image to the mouse pointer during a drag operation
        pub generated_image: RefCell<Option<DynamicImage>>,
        pub settings: gio::Settings,

        pub regeneration_lock: Arc<Cell<usize>>,
        pub app_busy: Arc<()>,

        pub drag_active: Rc<Cell<bool>>,
        pub file_properties: RefCell<FileProperties>,
        pub drag_cancelled: Cell<bool>,
    }

    impl Default for IconicWindow {
        fn default() -> Self {
            Self {
                toolbar: TemplateChild::default(),
                header_bar: TemplateChild::default(),
                toast_overlay: TemplateChild::default(),
                open_top_icon: TemplateChild::default(),
                image_view: TemplateChild::default(),
                save_button: TemplateChild::default(),
                threshold_scale: TemplateChild::default(),
                reset_color: TemplateChild::default(),
                monochrome_action_row: TemplateChild::default(),
                monochrome_color: TemplateChild::default(),
                scale_row: TemplateChild::default(),
                monochrome_switch: TemplateChild::default(),
                image_preferences: TemplateChild::default(),
                regeneration_osd: TemplateChild::default(),
                regeneration_osd_second: TemplateChild::default(),
                x_scale: TemplateChild::default(),
                y_scale: TemplateChild::default(),
                size: TemplateChild::default(),
                stack: TemplateChild::default(),
                main_status_page: TemplateChild::default(),
                monochrome_invert: TemplateChild::default(),
                image_loading_spinner: TemplateChild::default(),
                popover_menu: TemplateChild::default(),
                gesture_click: TemplateChild::default(),
                regeneration_revealer: TemplateChild::default(),
                drag_overlay: TemplateChild::default(),
                image_menu: TemplateChild::default(),
                image_menu_simple: TemplateChild::default(),
                bottom_image_file: Arc::new(Mutex::new(None)),
                top_image_file: Arc::new(Mutex::new(None)),
                saved_file: Arc::new(Mutex::new(None)),
                image_saved: Cell::new(true),
                generated_image: RefCell::new(None),
                file_created: Cell::new(false),
                settings: gio::Settings::new(APP_ID),
                default_colors: RefCell::new(HashMap::new()),
                last_drag_n_drop_generated_name: RefCell::new(None),
                regeneration_lock: Arc::new(Cell::new(0)),
                app_busy: Arc::new(()),
                drag_active: Rc::new(Cell::new(false)),
                file_properties: RefCell::new(FileProperties::default()),
                drag_cancelled: Cell::new(false),
                custom_mask: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IconicWindow {
        const NAME: &'static str = "IconicWindow";
        type Type = super::IconicWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);
            set_up_klass_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for IconicWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let imp = obj.imp();
            // If you read this and think of a more elegant way to achieve this, please let me know
            // This shares the value of if a drag operation is currently active
            // I now create a RefCell<Rc<Cell<bool>>>
            // Where i replace the Rc<> with the same as in this file

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            setup_drag_drop_logic(self);

            set_up_stateful_actions(imp);
        }

        fn dispose(&self) {
            self.dispose_template();
        }
    }
    impl WidgetImpl for IconicWindow {}

    impl WindowImpl for IconicWindow {
        fn close_request(&self) -> glib::Propagation {
            warn!("close request");
            let window = self.obj();
            // If iconic is busy, show busy pop-up
            if Arc::strong_count(&self.app_busy) >= 2 {
                window.force_quit_dialog_async_wrapper();
            // Else, show quit iconic, by showing save dialog if needed
            } else {
                window.quit_iconic();
            }
            // Closing the window is handled in the above two functions, so by returning stop. It won't quit the application from this function
            glib::Propagation::Stop
        }
    }
    impl ApplicationWindowImpl for IconicWindow {}
    impl AdwApplicationWindowImpl for IconicWindow {}
}

glib::wrapper! {
    pub struct IconicWindow(ObjectSubclass<imp::IconicWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements
            gio::ActionGroup,
            gio::ActionMap,
            gtk::Accessible,
            gtk::Buildable,
            gtk::ConstraintTarget,
            gtk::Native,
            gtk::Root,
            gtk::ShortcutManager;
}

impl IconicWindow {
    pub fn new<P: IsA<adw::Application>>(application: &P) -> Self {
        let win = glib::Object::builder::<IconicWindow>()
            .property("application", application)
            .build();
        let imp = win.imp();
        if PROFILE == "Devel" {
            imp.main_status_page.set_icon_name(Some(APP_ICON));
        }
        imp.default_colors.replace(HashMap::from([
            ("Blue".to_string(), RGBA::from_rgb(67, 141, 230)),
            ("Teal".to_string(), RGBA::from_rgb(18, 158, 176)),
            ("Green".to_string(), RGBA::from_rgb(61, 158, 79)),
            ("Yellow".to_string(), RGBA::from_rgb(203, 147, 26)),
            ("Orange".to_string(), RGBA::from_rgb(241, 119, 56)),
            ("Red".to_string(), RGBA::from_rgb(232, 64, 83)),
            ("Pink".to_string(), RGBA::from_rgb(230, 67, 146)),
            ("Purple".to_string(), RGBA::from_rgb(149, 74, 181)),
            ("Slate".to_string(), RGBA::from_rgb(99, 118, 146)),
        ]));
        win.setup_defaults();
        win
    }

    pub fn default_sliders(&self, add_marks: bool) {
        let imp = self.imp();
        if add_marks {
            imp.x_scale
                .add_mark(DEFAULT_X_SLIDER, gtk::PositionType::Top, None);
            imp.y_scale
                .add_mark(DEFAULT_X_SLIDER, gtk::PositionType::Bottom, None);
            imp.y_scale
                .add_mark(DEFAULT_Y_SLIDER, gtk::PositionType::Bottom, None);
            imp.size
                .add_mark(DEFAULT_SIZE_SLIDER, gtk::PositionType::Top, None);
        }
        imp.y_scale.set_value(DEFAULT_Y_SLIDER);
        imp.size.set_value(DEFAULT_SIZE_SLIDER);
        imp.x_scale.set_value(DEFAULT_X_SLIDER);
        let monochrome_switch_state = imp.settings.boolean("monochrome-mode-active");
        imp.monochrome_switch.set_active(monochrome_switch_state);
    }

    pub fn setup_defaults(&self) {
        let imp = self.imp();
        imp.save_button.set_sensitive(false);
        self.default_sliders(true);
        self.adwaita_colors_popup();
        imp.reset_color.set_visible(false);
        self.check_if_regeneration_needed();
        let _ = imp
            .settings
            .set_string("previous-system-accent-color", &self.get_accent_color());
        imp.stack.set_visible_child_name("stack_welcome_page");
        self.setup_settings();
        self.setup_update();
        self.set_up_and_load_bottom_icon();
        self.slider_control_sensitivity(false);
    }

    pub fn setup_settings(&self) {
        let imp = self.imp();
        let update_folder = glib::clone!(
            #[weak(rename_to = win)]
            self,
            move |_: &gio::Settings, _: &str| {
                win.set_up_and_load_bottom_icon();
            }
        );

        adw::StyleManager::default().connect_accent_color_notify(glib::clone!(
            #[weak(rename_to = win)]
            self,
            move |_| {
                let imp = win.imp();
                if imp.file_properties.borrow().bottom_image_type == BottomImageType::FolderSystem {
                    // error!("Reloading folder image");
                    win.check_if_regeneration_needed();
                    win.set_up_and_load_bottom_icon();
                }
                _ = imp
                    .settings
                    .set_string("previous-system-accent-color", &win.get_accent_color());
            }
        ));

        imp.settings
            .connect_changed(Some("folder-svg-path"), update_folder.clone());
        imp.settings
            .connect_changed(Some("primary-folder-color"), update_folder.clone());
        imp.settings
            .connect_changed(Some("secondary-folder-color"), update_folder.clone());
        imp.settings
            .connect_changed(Some("selected-accent-color"), update_folder.clone());
        imp.settings
            .connect_changed(Some("manual-bottom-image-selection"), update_folder.clone());
    }

    pub fn setup_update(&self) {
        self.imp().monochrome_invert.connect_active_notify(clone!(
            #[weak(rename_to = win)]
            self,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[weak]
                    win,
                    async move {
                        win.render_to_screen().await;
                    }
                ));
            }
        ));
        self.imp().monochrome_color.connect_rgba_notify(clone!(
            #[weak(rename_to = win)]
            self,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[weak]
                    win,
                    async move {
                        let imp = win.imp();
                        if imp.monochrome_color.rgba() != win.get_default_color() {
                            imp.reset_color.set_visible(true);
                        }

                        // TODO: I do not like this approach, but it works
                        if imp.stack.visible_child_name() == Some("stack_main_page".into()) {
                            win.render_to_screen().await;
                        }
                    }
                ));
            }
        ));
    }

    pub fn image_save_sensitive(&self, sensitive: bool) {
        let imp = self.imp();
        imp.image_saved.replace(!sensitive);
        imp.save_button.set_sensitive(sensitive);
    }

    pub fn reset_colors(&self) {
        let imp = self.imp();
        imp.monochrome_color.set_rgba(&self.get_default_color());
        imp.reset_color.set_visible(false);
    }

    // TODO: This approach is dumb. I am purposely failing a dictionary lookup and using unwrap_or to get my way
    pub fn get_default_color(&self) -> gdk::RGBA {
        let imp = self.imp();
        debug!("Resetting top color");
        let bottom_image_type = imp
            .file_properties
            .try_borrow()
            .unwrap()
            .bottom_image_type
            .clone();

        let accent_color = match bottom_image_type {
            BottomImageType::FolderSystem => self.get_accent_color(),
            BottomImageType::Folder(color) => color,
            BottomImageType::FolderCustom(_, bg) => bg,
            _ => "Blue".to_owned(),
        };

        let color = imp
            .default_colors
            .borrow()
            .get(&accent_color)
            .unwrap_or(&RGBA::from_hex(accent_color))
            .clone();
        debug!("Found color: {:?}", &color);
        color
    }

    // Get the current accent color
    pub fn get_accent_color(&self) -> String {
        let accent_color = format!("{:?}", adw::StyleManager::default().accent_color());
        accent_color
    }

    // Find the custom bottom image in the cache
    pub fn check_chache_icon(&self, file_name: &str) -> PathBuf {
        let imp = self.imp();
        let icon_path = PathBuf::from(&imp.settings.string("folder-svg-path"));
        let cache_path = user_cache_dir();
        let folder_icon_cache_path = cache_path.join(file_name);
        if folder_icon_cache_path.exists() {
            info!("File found in cache at: {:?}", folder_icon_cache_path);
            return folder_icon_cache_path;
        } else if icon_path.exists() {
            info!(
                "File not found in cache, copying to: {:?}",
                folder_icon_cache_path
            );
            return self
                .copy_folder_image_to_cache(&icon_path, &cache_path)
                .unwrap()
                .0;
        }
        info!("File not found AT ALL");
        let _ = show_error_popup(
            &self,
            &gettext("The set bottom icon could not be found, resetting"),
            false,
            None::<String>,
        )
        .unwrap();

        imp.settings.default_value("manual-bottom-image-selection");
        self.get_built_in_bottom_icon_path("None")
    }

    // This checks if the main page, or welcome screen needs to be shown. And adds ability to loads just a bottom file
    // TODO: This function is REALLY confusing and needs to be rewritten
    pub fn check_icon_update(&self) {
        let imp = self.imp();
        let mut top_image = imp.top_image_file.lock().unwrap();
        let bottom_image = imp.bottom_image_file.lock().unwrap();
        if (*top_image).is_some() && (*bottom_image).is_some() {
            let top_image_width = top_image.as_ref().unwrap().image.width();
            if top_image_width > 1 {
                // If the top image is empty, these controlls are disabled
                // This is to check if it's needed to turn them on again
                self.slider_control_sensitivity(true);
            }
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = win)]
                self,
                async move {
                    win.render_to_screen().await;
                }
            ));
            imp.image_loading_spinner.set_visible(false);
            imp.stack.set_visible_child_name("stack_main_page");
        } else if (*bottom_image).is_some() {
            debug!("Loaded temporary image for render");
            // Create image of nothing
            self.load_empty_top_image(&mut top_image);

            if imp.stack.visible_child_name() != Some("stack_main_page".into()) {
                imp.stack.set_visible_child_name("stack_welcome_page");
            }
        }
    }

    pub fn load_empty_top_image(&self, top_image: &mut std::sync::MutexGuard<'_, Option<File>>) {
        let empty_image = DynamicImage::new(1, 1, ColorType::Rgba8);
        (*top_image).replace(File::from_image(empty_image, 1, None, ""));
        self.slider_control_sensitivity(false);
    }

    pub fn slider_control_sensitivity(&self, sensitive: bool) {
        let imp = self.imp();
        imp.x_scale.set_sensitive(sensitive);
        imp.y_scale.set_sensitive(sensitive);
        imp.scale_row.set_sensitive(sensitive);
        imp.threshold_scale.set_sensitive(sensitive);
        imp.monochrome_color.set_sensitive(sensitive);
        imp.monochrome_invert.set_sensitive(sensitive);
        imp.monochrome_switch.set_sensitive(sensitive);
        imp.monochrome_action_row.set_sensitive(sensitive);
    }

    pub async fn open_file_chooser(&self) -> Option<gio::File> {
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let filter = gtk::FileFilter::new();
        filter.add_mime_type("image/*");
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Open Document"))
            .modal(true)
            .filters(&filters)
            .build();
        let file = dialog.open_future(Some(self)).await;
        match file {
            Ok(file) => {
                debug!("{:?}", &file.path().unwrap());
                Some(file)
            }
            Err(error) => {
                error!("{:?}", error);
                None
            }
        }
    }

    // opens file explorer to location of saved icon
    pub async fn open_directory(&self, file: &gio::File) {
        let launcher = gtk::FileLauncher::new(Some(file));
        let win = self.native().and_downcast::<gtk::Window>();
        if let Err(e) = launcher.open_containing_folder_future(win.as_ref()).await {
            show_error_popup(&self, "", true, Some(Box::new(e)));
        };
    }

    pub fn dynamic_image_to_texture(&self, dynamic_image: &DynamicImage) -> gdk::Texture {
        let rgba_image = dynamic_image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        let pixels = rgba_image.into_raw(); // Get the raw pixel data
        // Create Pixbuf from raw pixel data
        MemoryTexture::new(
            width as i32,
            height as i32,
            gdk4::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from(&pixels),
            width as usize * 4,
        )
        .upcast()
    }

    // TODO decouple UI components from these functions
    pub fn monochrome_switch_change(&self) {
        let imp = self.imp();
        let switch_state = imp.monochrome_switch.is_active();
        debug!("Updating monochrome state to {:?}", switch_state);
        _ = imp
            .settings
            .set_boolean("monochrome-mode-active", switch_state);
        if self.imp().stack.visible_child_name() == Some("stack_main_page".into()) {
            self.check_icon_update();
        }
    }
}
