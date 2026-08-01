use std::sync::Arc;

use crate::{
    config::PROFILE,
    objects::{
        errors::show_error_popup,
        properties::{BottomImageType, MaskType},
    },
    window::imp::IconicWindow,
};
use adw::subclass::prelude::*;
use adw::{Toast, prelude::*};
use gettextrs::gettext;
use gio::{
    SimpleAction,
    glib::{VariantTy, clone, subclass::basic::ClassStruct, user_cache_dir},
};
use gtk::glib;
use log::*;

pub fn set_up_stateful_actions(window: &IconicWindow) {
    let obj = window.obj();

    let temp_bottom_action = set_up_temp_bottom_icon_action(window);
    let mask_action = set_up_mask_action(window);
    let custom_mask_action = set_up_custom_mask_action(window);
    obj.add_action(&mask_action);
    obj.add_action(&temp_bottom_action);
    obj.add_action(&custom_mask_action);
}

fn set_up_temp_bottom_icon_action(window: &IconicWindow) -> SimpleAction {
    let temp_bottom_folder = SimpleAction::new_stateful(
        "temp_folder_color",
        Some(&VariantTy::STRING),
        &"".to_variant(),
    );

    temp_bottom_folder.connect_change_state(clone!(
        #[weak (rename_to=imp)]
        window,
        move |action, para| {
            let obj = imp.obj();
            let value = para.unwrap().str().unwrap().to_owned();
            debug!("{value}");
            if value != "" {
                let mut properties = imp.file_properties.try_borrow().unwrap().clone();
                properties.bottom_image_type = match value.as_str() {
                    "Custom" => {
                        let custom_primary_color: String =
                            imp.settings.string("primary-folder-color").into();
                        let custom_secondary_color: String =
                            imp.settings.string("secondary-folder-color").into();
                        BottomImageType::FolderCustom(custom_primary_color, custom_secondary_color)
                    }
                    _ => BottomImageType::Folder(value),
                };
                imp.file_properties.replace(properties);
                obj.load_bottom_image();
            }
            action.set_state(para.unwrap());
        }
    ));
    temp_bottom_folder
}

fn set_up_mask_action(window: &IconicWindow) -> SimpleAction {
    let mask_enabled = &window.settings.boolean("mask-enabled").to_variant();
    let mask_action = SimpleAction::new_stateful("enable-mask", None, mask_enabled);

    mask_action.connect_activate(clone!(
        #[weak (rename_to=imp)]
        window,
        move |action, _para| {
            let mut properties = imp.file_properties.try_borrow().unwrap().clone();
            let obj = imp.obj();

            properties.mask = match action.state() {
                Some(t) if t == true.to_variant() => {
                    action.set_state(&false.to_variant());
                    MaskType::Disabled
                }
                _ => {
                    action.set_state(&true.to_variant());
                    MaskType::Automatic
                }
            };
            imp.file_properties.replace(properties);
            obj.check_icon_update();
        }
    ));
    mask_action
}

fn set_up_custom_mask_action(window: &IconicWindow) -> SimpleAction {
    let mask_action = SimpleAction::new_stateful("custom-mask", None, &false.to_variant());
    mask_action.connect_activate(clone!(
        #[weak]
        window,
        move |_, _| {
            glib::spawn_future_local(clone!(
                #[weak]
                window,
                async move {
                    let obj = window.obj();
                    _ = obj.choose_custom_mask().await;
                    obj.check_icon_update();
                }
            ));
        }
    ));
    mask_action
}

/*
    -------------------------------------------------------------------------------------------------------------------------------------------------------------

    Split between statefull and klass actions

    -------------------------------------------------------------------------------------------------------------------------------------------------------------
*/

pub fn set_up_klass_actions(klass: &mut ClassStruct<IconicWindow>) {
    klass.install_action("app.open_top_icon", None, move |win, _, _| {
        glib::spawn_future_local(clone!(
            #[weak]
            win,
            async move {
                win.load_top_icon().await;
            }
        ));
        debug!("References: {}", Arc::strong_count(&win.imp().app_busy));
    });
    klass.install_action("app.open_file_location", None, move |win, _, _| {
        glib::spawn_future_local(clone!(
            #[weak]
            win,
            async move {
                let file = win.imp().saved_file.lock().unwrap().clone().unwrap();
                win.open_directory(&file).await;
            }
        ));
    });
    klass.install_action("app.select_folder", None, move |win, _, _| {
        glib::spawn_future_local(clone!(
            #[weak]
            win,
            async move {
                win.load_temp_folder_icon().await;
            }
        ));
    });
    klass.install_action("app.open_empty_bottom", None, move |win, _, _| {
        win.check_icon_update();
    });
    klass.install_action("app.advanced", None, move |win, _, _| {
        let imp = win.imp();
        let advanced_state = imp.settings.boolean("advanced-settings");
        let new_state = if advanced_state {
            imp.toast_overlay
                .add_toast(Toast::new(&gettext("Disabled advanced mode")));
            false
        } else {
            imp.toast_overlay
                .add_toast(Toast::new(&gettext("Enabled advanced mode")));
            true
        };
        _ = imp.settings.set_boolean("advanced-settings", new_state);
    });
    klass.install_action("app.reset", None, move |win, _, _| {
        let imp = win.imp();
        win.default_sliders(false);
        win.set_up_and_load_bottom_icon();
        let mut top_image = imp.top_image_file.lock().unwrap();
        win.load_empty_top_image(&mut top_image);
        imp.toast_overlay
            .add_toast(adw::Toast::new(&gettext("Image reset")));
    });
    klass.install_action("app.reset_bottom", None, move |win, _, _| {
        win.reset_bottom_icon();
    });
    klass.install_action("app.paste", None, move |win, _, _| {
        glib::spawn_future_local(clone!(
            #[weak]
            win,
            async move {
                win.paste_from_clipboard().await;
            }
        ));
    });
    klass.install_action("app.regenerate", None, move |win, _, _| {
        glib::spawn_future_local(clone!(
            #[weak]
            win,
            async move {
                let imp = win.imp();
                let id = imp.regeneration_lock.get();
                imp.regeneration_lock.replace(id + 1);
                match win.regenerate_icons().await {
                    Ok(_) => (),
                    Err(x) => {
                        show_error_popup(&win, "", true, Some(x));
                    }
                };
                //imp.stack.set_visible_child_name(&previous_stack);
                debug!("Done generating");
            }
        ));
    });
    klass.install_action("app.save_button", None, move |win, _, _| {
        glib::spawn_future_local(clone!(
            #[weak]
            win,
            async move {
                win.drag_and_drop_information_dialog();
                match win.open_save_file_dialog().await {
                    Ok(_) => (),
                    Err(error) => {
                        show_error_popup(&win, &error.to_string(), true, Some(error));
                    }
                };
            }
        ));
    });
    klass.install_action("app.monochrome_switch", None, move |win, _, _| {
        win.monochrome_switch_change();
    });
    klass.install_action("app.reset_color", None, move |win, _, _| {
        win.reset_colors();
    });

    klass.install_action("app.debug_mask", None, move |win, _, _| {
        let imp = win.imp();
        let mut cache_path = user_cache_dir();
        cache_path.push("mask.png");
        let mask = imp.bottom_image_file.lock().unwrap().clone();
        if let Some(mask) = mask {
            _ = mask.image_mask.is_some_and(|mask| {
                _ = mask.save(&cache_path);
                true
            });
        }
        imp.toast_overlay
            .add_toast(adw::Toast::new(&format!("mask saved at {:?}", cache_path)));
    });

    // DEBUG
    // --------------------------------------------------------------------------------
    if PROFILE == "Devel" {
        klass.install_action("app.debug", None, move |win, _, _| {
            let imp = win.imp();
            let properties = imp.file_properties.borrow().clone();
            println!("{:#?}", properties);
        });
    }
}
