/* application.rs
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

use crate::IconicWindow;
use crate::config::{APP_ICON, PROFILE, VERSION};
use crate::glib::WeakRef;
use crate::settings::settings::PreferencesDialog;
use adw::prelude::{AdwApplicationWindowExt, AdwDialogExt};
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::License;
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::OnceCell;

mod imp {

    use super::*;

    #[derive(Debug, Default)]
    pub struct IconicApplication {
        pub window: OnceCell<WeakRef<IconicWindow>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IconicApplication {
        const NAME: &'static str = "IconicApplication";
        type Type = super::IconicApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for IconicApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_gactions();
            obj.setup_accels();
        }
    }

    impl ApplicationImpl for IconicApplication {
        // We connect to the activate callback to create a window when the application
        // has been launched. Additionally, this callback notifies us when the user
        // tries to launch a "second instance" of the application. When they try
        // to do that, we'll just present any existing window.
        fn activate(&self) {
            let application = self.obj();
            // Get the current window or create one if necessary
            let window = if let Some(window) = application.active_window() {
                window
            } else {
                let window = IconicWindow::new(&*application);
                window.upcast()
            };
            // Ask the window manager/compositor to present the window
            window.present();
        }
    }

    impl GtkApplicationImpl for IconicApplication {}
    impl AdwApplicationImpl for IconicApplication {}
}

glib::wrapper! {
    pub struct IconicApplication(ObjectSubclass<imp::IconicApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl IconicApplication {
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .build()
    }

    fn setup_gactions(&self) {
        let quit_action = gio::ActionEntry::builder("quit")
            .activate(move |app: &Self, _, _| app.quit())
            .build();
        let about_action = gio::ActionEntry::builder("about")
            .activate(move |app: &Self, _, _| app.show_about())
            .build();
        let settings_action = gio::ActionEntry::builder("preferences")
            .activate(move |app: &Self, _, _| app.show_preferences_dialog())
            .build();
        let open_action = gio::ActionEntry::builder("open top")
            .activate(move |app: &Self, _, _| app.open_function())
            .build();
        let open_folder_action = gio::ActionEntry::builder("open folder")
            .activate(move |app: &Self, _, _| app.open_folder_function())
            .build();
        let paste_action = gio::ActionEntry::builder("paste")
            .activate(move |app: &Self, _, _| app.paste_image())
            .build();
        let shortcuts_action = gio::ActionEntry::builder("shortcuts")
            .activate(move |app: &Self, _, _| app.shortcuts())
            .build();
        self.add_action_entries([
            quit_action,
            about_action,
            settings_action,
            open_action,
            open_folder_action,
            paste_action,
            shortcuts_action,
        ]);
    }

    fn setup_accels(&self) {
        self.set_accels_for_action("app.save_button", &["<primary>s"]);
        self.set_accels_for_action("app.open_top_icon", &["<primary>o"]);
        self.set_accels_for_action("app.quit", &["<primary>q"]);
        self.set_accels_for_action("app.preferences", &["<primary>comma"]);
        self.set_accels_for_action("app.select_folder", &["<primary><shift>o"]);
        self.set_accels_for_action("app.paste", &["<primary>v"]);
        self.set_accels_for_action("app.advanced", &["<primary>a"]);
        self.set_accels_for_action("app.regenerate", &["<primary>r"]);
        self.set_accels_for_action("app.shortcuts", &["<primary>question"]);
        self.set_accels_for_action("app.reset", &["r"]);
        self.set_accels_for_action("app.debug_mask", &["<primary>m"]);
        if PROFILE == "Devel" {
            self.set_accels_for_action("app.debug", &["a"]);
        }
    }

    fn show_preferences_dialog(&self) {
        let preferences = PreferencesDialog::new();
        let window = self.active_window().unwrap();
        let adw_window = window.downcast_ref::<IconicWindow>().unwrap();
        match adw_window.visible_dialog() {
            Some(dialog) => {
                dialog.close();
            }
            None => adw::prelude::AdwDialogExt::present(&preferences, Some(&window)),
        }
    }

    fn open_function(&self) {
        self.activate_action("app.open_top_icon", None);
    }

    fn open_folder_function(&self) {
        self.activate_action("app.select_folder", None);
    }

    fn paste_image(&self) {
        self.activate_action("app.paste", None);
    }

    fn shortcuts(&self) {
        let window = self.active_window().unwrap();
        let dialog = gtk::Builder::from_resource("/nl/emphisia/icon/gtk/shortcuts-dialog.ui")
            .object::<adw::ShortcutsDialog>("shortcuts_dialog")
            .unwrap();
        dialog.present(Some(&window));
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();

        let about = adw::AboutDialog::builder()
            .application_name("Iconic")
            .application_icon(APP_ICON)
            .developer_name("Youpie")
            .developers(vec![
                "Youpie https://github.com/youpie",
                "Qustio https://github.com/Qustio",
            ])
            .comments(&gettext(
                "Iconic allows you to add images onto a folder.
You can drag these created images straight into the properties window of the file manager!

If you like this app, feel free to donate, I would really appreciate it!",
            ))
            // Translators: This should not be translated, Please enter your credits here instead (format: "Name https://example.com" or "Name <email@example.com>", no quotes)
            .translator_credits(gettext("translator-credits"))
            .version(VERSION)
            .issue_url("https://github.com/youpie/Iconic/issues")
            .website("https://github.com/youpie/Iconic")
            .license_type(License::Gpl30)
            .copyright("© 2026 YoupDeGamerNL\nPlease note that I am proud of you and think you are good enough as you are!!!")
            .build();
        about.add_credit_section(Some("Supporters of iconic"), &["Nido", "Jala015"]);
        about.add_acknowledgement_section(
            Some("Code inspiration from"),
            &[
                "Eyedropper https://github.com/FineFindus/eyedropper",
                "Switcheroo https://gitlab.com/adhami3310/Switcheroo",
                "Geopard https://github.com/ranfdev/Geopard",
                "Obfuscate https://gitlab.gnome.org/World/obfuscate",
                "Loupe https://gitlab.gnome.org/GNOME/loupe",
            ],
        );
        about.add_link("Donate ❤️", "https://ko-fi.com/youpie");
        about.add_acknowledgement_section(
            Some("Icon inspiration from"),
            &[
                "Gnome text editor https://gitlab.gnome.org/GNOME/gnome-text-editor",
                "Adwaita icons https://gitlab.gnome.org/GNOME/adwaita-icon-theme",
            ],
        );
        about.add_acknowledgement_section(
            Some("Folder credits"),
            &["Adwaita-Colors https://github.com/dpejoh/Adwaita-colors/tree/main"],
        );
        about.present(Some(&window));
    }
}
