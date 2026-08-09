use crate::IconicWindow;
use crate::glib;
use crate::objects::properties::BottomImageType;
use crate::objects::properties::MaskType;
use adw::subclass::prelude::*;
use gio::glib::object::Cast;
use gio::glib::object::ObjectExt;
use gio::glib::variant::ToVariant;
use gio::prelude::ActionExt;
use gio::prelude::ActionMapExt;
use gio::prelude::SettingsExt;
use gtk::GestureClick;
use gtk::GestureLongPress;
use gtk::gdk;
use gtk::prelude::PopoverExt;
use log::debug;

#[gtk::template_callbacks]
impl IconicWindow {
    #[template_callback]
    pub async fn render_callback(&self, _hoi: glib::Object) {
        if self.imp().stack.visible_child_name() == Some("stack_main_page".into()) {
            self.render_to_screen().await;
        } else {
            debug!("not main stack");
        }
    }

    #[template_callback]
    pub async fn open_popover(&self, #[rest] values: &[glib::Value]) {
        let imp = self.imp();
        if let Some(gesture) = values.last().and_then(|x| x.get::<glib::Object>().ok()) {
            let (x, y) = if gesture.downcast_ref::<GestureLongPress>().is_some() {
                debug!("Longpress");
                (
                    values[0].get::<f64>().unwrap_or_default(),
                    values[1].get::<f64>().unwrap_or_default(),
                )
            } else if gesture.downcast_ref::<GestureClick>().is_some() {
                debug!("Click");
                (
                    values[1].get::<f64>().unwrap_or_default(),
                    values[2].get::<f64>().unwrap_or_default(),
                )
            } else {
                debug!("Neither, {}", gesture.type_().name());
                (0.0, 0.0)
            };

            let value = match imp.file_properties.borrow().bottom_image_type.clone() {
                BottomImageType::FolderSystem => self.get_accent_color().to_variant(),
                BottomImageType::Folder(color) => color.to_variant(),
                BottomImageType::FolderCustom(_, _) => "Custom".to_variant(),
                _ => "".to_variant(),
            };
            if let Some(action) = self.lookup_action("temp_folder_color")
                && Some(value.clone()) != action.state()
            {
                debug!("Changed value to {value:?}");
                action.change_state(&value);
            } else {
                debug!("Action not found");
            }

            if let Some(action) = self.lookup_action("enable-mask")
                && let Some(custom) = self.lookup_action("custom-mask")
            {
                let mask_type = imp.file_properties.try_borrow().unwrap().mask.clone();
                match mask_type {
                    MaskType::Disabled => {
                        action.change_state(&false.to_variant());
                        custom.change_state(&false.to_variant())
                    }
                    MaskType::Automatic => {
                        action.change_state(&true.to_variant());
                        custom.change_state(&false.to_variant())
                    }
                    MaskType::Custom => {
                        action.change_state(&true.to_variant());
                        custom.change_state(&true.to_variant())
                    }
                }
            }

            let position = gdk::Rectangle::new(x as i32, y as i32, 0, 0);
            if imp.settings.boolean("advanced-settings") {
                imp.popover_menu.set_menu_model(Some(&imp.image_menu.get()));
            } else {
                imp.popover_menu
                    .set_menu_model(Some(&imp.image_menu_simple.get()));
            }
            imp.popover_menu.set_pointing_to(Some(&position));
            imp.popover_menu.popup();
        }
    }
}
