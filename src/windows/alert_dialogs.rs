use std::path::PathBuf;

use crate::objects::errors::show_error_popup;

use adw::AlertDialog;
use adw::prelude::Cast;
use adw::prelude::{AdwApplicationWindowExt, AdwDialogExt, AlertDialogExt, AlertDialogExtManual};
use gettextrs::gettext;
use gio::glib::clone;
use gio::{
    glib,
    prelude::{ActionGroupExt, SettingsExt, SettingsExtManual},
    subclass::prelude::ObjectSubclassIsExt,
};
use gtk::prelude::GtkWindowExt;
use log::*;

use crate::IconicWindow;

impl IconicWindow {
    pub fn drag_and_drop_information_dialog(&self) {
        let imp = self.imp();
        if imp.settings.boolean("drag-and-drop-popup-shown") {
            return ();
        }
        const RESPONSE_OK: &str = "OK";
        let dialog = adw::AlertDialog::builder()
                .heading(&gettext("Drag and drop"))
                .body(&gettext("Did you know that it is possible to drag the folder image straight out of Iconic and drop it into nautilus' property window.\nNo need to save!"))
                .default_response(RESPONSE_OK)
                .build();
        dialog.add_response(RESPONSE_OK, &gettext("OK"));
        dialog.present(Some(self));
        let _ = imp.settings.set("drag-and-drop-popup-shown", true);
    }

    pub async fn top_or_bottom_popup(&self) -> Option<bool> {
        let dnd_switch_state = self.imp().settings.boolean("default-dnd-activated");
        let dnd_radio_state = self.imp().settings.string("default-dnd-action");
        debug!("radio button state: {}", dnd_radio_state);
        if dnd_switch_state {
            return match dnd_radio_state.as_str() {
                "top" => Some(true),
                "bottom" => Some(false),
                _ => None,
            };
        }
        const RESPONSE_TOP: &str = "TOP";
        const RESPONSE_BOTTOM: &str = "BOTTOM";
        let load_question: &str =
            &gettext("Do you want to load this image to the top or bottom layer?");
        let disable_hint: &str = &gettext("Hint: You can disable this pop-up in the settings");
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Select layer"))
            .body(format!("{load_question} \n <sub> <span foreground=\"#9A9996\"> {disable_hint}</span> </sub>"))
            .body_use_markup(true)
            .default_response(RESPONSE_TOP)
            .build();
        dialog.add_response(RESPONSE_TOP, &gettext("Top"));
        dialog.add_response(RESPONSE_BOTTOM, &gettext("Bottom"));
        dialog.set_response_appearance(RESPONSE_TOP, adw::ResponseAppearance::Suggested);

        match &*dialog.clone().choose_future(Some(self)).await {
            RESPONSE_TOP => Some(true),
            RESPONSE_BOTTOM => Some(false),
            _ => None,
        }
    }

    pub async fn confirm_save_changes(&self) -> bool {
        let mut quit_iconic = false;
        const RESPONSE_CANCEL: &str = "cancel";
        const RESPONSE_DISCARD: &str = "discard";
        const RESPONSE_SAVE: &str = "save";
        const RESPONSE_CLOSE: &str = "close";
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Save Changes?"))
            .body(gettext("Open image contain unsaved changes. Changes which are not saved will be permanently lost"))
            .close_response(RESPONSE_CLOSE)
            .default_response(RESPONSE_SAVE)
            .build();
        dialog.add_response(RESPONSE_CANCEL, &gettext("Cancel"));
        dialog.add_response(RESPONSE_DISCARD, &gettext("Discard"));
        dialog.set_response_appearance(RESPONSE_DISCARD, adw::ResponseAppearance::Destructive);
        dialog.add_response(RESPONSE_SAVE, &gettext("Save"));
        dialog.set_response_appearance(RESPONSE_SAVE, adw::ResponseAppearance::Suggested);
        match &*dialog.clone().choose_future(Some(self)).await {
            RESPONSE_CANCEL => {
                dialog.close();
            }
            RESPONSE_DISCARD => {
                quit_iconic = true;
            }
            RESPONSE_SAVE => match self.open_save_file_dialog().await {
                Ok(saved) => match saved {
                    true => {
                        quit_iconic = true;
                    }
                    false => (),
                },
                Err(error) => {
                    show_error_popup(&self, &error.to_string(), true, Some(error));
                }
            },
            RESPONSE_CLOSE => {
                self.imp().image_saved.replace(true);
                dialog.close();
            }
            _ => unreachable!(),
        }
        if quit_iconic {
            self.application().unwrap().activate_action("quit", None)
        }
        false
    }

    pub fn adwaita_colors_popup(&self) {
        glib::spawn_future_local(clone!(
            #[weak (rename_to = win)]
            self,
            async move {
                let imp = win.imp();
                let adwaita_colors_path =
                    PathBuf::from("/run/host/usr/share/icons/Adwaita-blue/index.theme");
                let pop_up_already_shown = imp.settings.boolean("adwaita-colors-dialog-shown");
                let automatic_folder_color_selected =
                    imp.settings.string("selected-accent-color") == "None";
                if adwaita_colors_path.exists()
                    && !pop_up_already_shown
                    && !automatic_folder_color_selected
                {
                    match win.show_alert_dialog(&gettext("Adwaita Colors detected"), 
                    &gettext("Adwaita colors has been detected on your system, do you want to enable the folder color to adapt to your active system accent color?"), 
                    vec![&gettext("Enable"), &gettext("Don't enable")]).await {
                        Some(0) => {_ = imp.settings.set_string("selected-accent-color", "None")},
                        Some(1) => (),
                        _ => ()
                    };
                    _ = imp
                        .settings
                        .set_boolean("adwaita-colors-dialog-shown", true);
                }
            }
        ));
    }

    pub fn drag_and_drop_regeneration_popup(&self) {
        let imp = self.imp();
        if !imp.settings.boolean("regeneration-hint-shown")
            && imp
                .file_properties
                .borrow()
                .bottom_image_type
                .is_regeneration_compatible()
                == Some(true)
        {
            const RESPONSE_OK: &str = "OK";
            let dialog = adw::AlertDialog::builder()
                .heading(&gettext("Regenerating Icons"))
                .body(&gettext("If you drag and drop icons and change your accent color. It is then possible to regenerate the images by pressing \"regenerate\" in the menu or by pressing ctrl+R"))
                .default_response(RESPONSE_OK)
                .build();
            dialog.add_response(RESPONSE_OK, &gettext("OK"));
            dialog.present(Some(self));
            let _ = imp.settings.set("regeneration-hint-shown", true);
        }
    }

    pub fn force_quit_dialog_async_wrapper(&self) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=win)]
            self,
            async move { win.force_quit_dialog().await }
        ));
    }

    async fn force_quit_dialog(&self) {
        const RESPONSE_WAIT: &str = "WAIT_QUIT";
        const RESPONSE_FORCE_QUIT: &str = "QUIT";
        let dialog = adw::AlertDialog::builder()
                .heading(&gettext("Iconic is busy"))
                .body(&gettext("Iconic is currently busy, it is recommended to wait before closing to prevent data loss"))
                .default_response(RESPONSE_WAIT)
                .close_response(RESPONSE_WAIT)
                .build();
        // Aparently items appear reversed from how they are defined here
        dialog.add_response(RESPONSE_FORCE_QUIT, &gettext("Quit anyway"));
        dialog.set_response_appearance(RESPONSE_FORCE_QUIT, adw::ResponseAppearance::Destructive);
        dialog.add_response(RESPONSE_WAIT, &gettext("Wait"));
        dialog.set_response_appearance(RESPONSE_WAIT, adw::ResponseAppearance::Suggested);
        dialog.present(Some(self));

        match &*dialog.clone().choose_future(Some(self)).await {
            RESPONSE_WAIT => (),
            RESPONSE_FORCE_QUIT => self.application().unwrap().activate_action("quit", None),
            _ => unreachable!(),
        }
    }

    // The options must me unique!
    pub async fn show_alert_dialog(
        &self,
        title: &str,
        text: &str,
        options: Vec<&str>,
    ) -> Option<usize> {
        let first_option = if let Some(option) = options.first() {
            *option
        } else {
            return None;
        };
        let last_option = if let Some(option) = options.last() {
            *option
        } else {
            return None;
        };

        let dialog = adw::AlertDialog::builder()
            .heading(title)
            .body(text)
            .default_response(first_option)
            .close_response(last_option)
            .build();
        let mut options_clone = options.clone();
        options_clone.reverse();

        for option in options_clone {
            dialog.add_response(option, option);
        }
        if options.len() > 1 {
            dialog.set_response_appearance(first_option, adw::ResponseAppearance::Suggested);
        }
        let option_chosen = &*dialog.clone().choose_future(Some(self)).await;
        let option = options.iter().position(|n| n == &option_chosen);
        debug!("Options: {options:?}, Chosen: {option_chosen} - Result: {option:?}");
        option
    }

    fn get_current_alert_dialog(&self) -> Option<AlertDialog> {
        let dialog = match self.visible_dialog() {
            Some(dialog) => dialog,
            None => {
                info!("No dialog found");
                return None;
            }
        };
        dialog.downcast::<AlertDialog>().ok()
    }

    // If a user tries to close iconic while it is busy a pop-up is shown
    // But after it is done being busy it is nice to just close that pop up automatically
    pub fn close_iconic_busy_popup(&self) {
        if let Some(alert_dialog) = self.get_current_alert_dialog() {
            if alert_dialog.default_response() == Some("WAIT_QUIT".into()) {
                alert_dialog.close();
                warn!("Busy dialog is found, closing");
                self.quit_iconic();
            } else {
                info!("Dialog is found, but not busy dialog");
            }
        }
    }

    pub fn quit_iconic(&self) {
        let imp = self.imp();
        if imp.image_saved.get() {
            error!("closing iconic");
            self.application().unwrap().activate_action("quit", None);
        }
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to=win)]
            self,
            async move {
                win.confirm_save_changes().await;
            }
        ));
    }
}
