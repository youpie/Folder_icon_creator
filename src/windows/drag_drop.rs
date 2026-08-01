use crate::objects::errors::ErrorPopup;
use crate::{IconicWindow, window};
use crate::{glib::clone, objects::errors::show_error_popup};
use adw::subclass::prelude::*;
use gio::glib::user_data_dir;
use gio::{glib, prelude::*};
use gtk::gdk;
use gtk::prelude::WidgetExt;
use image::imageops;
use log::*;
use uuid::Uuid;

pub fn setup_drag_drop_logic(win: &window::imp::IconicWindow) {
    let obj = win.obj();
    obj.imp()
        .drag_overlay
        .imp()
        .current_drop_is_meta
        .replace(obj.imp().drag_active.clone());

    let drop_target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
    // drop_target.set_types(&[gdk::Texture::static_type(), gio::File::static_type()]);

    drop_target.connect_accept(clone!(
        #[strong]
        obj,
        move |target, drop| {
            let imp = obj.imp();
            if drop.formats().contain_mime_type("image/svg+xml") {
                info!("File contains SVG");
                target.set_types(&[gio::File::static_type(), gdk::Texture::static_type()]);
            } else {
                info!("File does not contain SVG");
                target.set_types(&[gdk::Texture::static_type(), gio::File::static_type()]);
            }
            if imp.drag_active.get() && !imp.settings.boolean("allow-meta-drop") {
                info!("Drag active, disabling target");
                target.set_actions(gdk::DragAction::empty());
            } else if imp.drag_active.get() && imp.settings.boolean("allow-meta-drop") {
                info!("Switching to File type");
                target.set_types(&[gio::File::static_type(), gdk::Texture::static_type()]);
                target.set_actions(gdk::DragAction::COPY);
            } else {
                target.set_actions(gdk::DragAction::COPY);
            }
            true
        }
    ));
    drop_target.connect_drop(clone!(
        #[strong]
        obj,
        move |_, value, _, _| {
            debug!("Value type: {}", value.type_().name());
            if let Ok(file) = value.get::<gio::File>() {
                glib::spawn_future_local(glib::clone!(
                    #[weak(rename_to = win)]
                    obj,
                    async move {
                        win.open_dragged_file(file).await;
                    }
                ));
                true
            } else if let Ok(texture) = value.get::<gdk::Texture>() {
                glib::spawn_future_local(glib::clone!(
                    #[weak(rename_to = win)]
                    obj,
                    async move {
                        win.open_dragged_texture(texture).await;
                    }
                ));
                true
            } else {
                false
            }
        }
    ));

    let drag_source = gtk::DragSource::builder()
        .actions(gdk::DragAction::COPY)
        .build();

    drag_source.connect_prepare(clone!(
        #[weak (rename_to = win)]
        obj,
        #[upgrade_or]
        None,
        move |drag, _, _| win.drag_connect_prepare(drag)
    ));

    drag_source.connect_drag_end(clone!(
        #[weak (rename_to = win)]
        obj,
        move |_, _, _| win.drag_connect_end()
    ));
    drag_source.connect_drag_cancel(clone!(
        #[weak (rename_to = win)]
        obj,
        #[upgrade_or]
        false,
        move |_, _, drag_cancel_reason| win.drag_connect_cancel(drag_cancel_reason)
    ));
    win.drag_overlay.set_drop_target(&drop_target);
    win.image_view.add_controller(drag_source);
}

impl IconicWindow {
    pub fn drag_connect_prepare(&self, source: &gtk::DragSource) -> Option<gdk::ContentProvider> {
        let imp = self.imp();
        imp.drag_active.set(true);
        let generated_image = imp.generated_image.borrow().clone().unwrap();
        let file_hash = imp.top_image_file.lock().unwrap().clone().unwrap().hash;
        let icon = self.dynamic_image_to_texture(&generated_image.resize(
            64,
            64,
            imageops::FilterType::Nearest,
        ));
        source.set_icon(Some(&icon), 0 as i32, 0 as i32);
        let gio_file = self.create_drag_file(false);
        let gio_file_temp = self.create_drag_file(true);
        imp.last_drag_n_drop_generated_name
            .replace(Some(gio_file.clone()));
        let gio_file_clone = gio_file.clone();
        // I think it is quite cursed what i am doing. But it works amazingly for speeding up drag responsiveness
        glib::spawn_future_local(clone!(
            #[weak (rename_to = win)]
            self,
            async move {
                _ = win
                    .save_file(
                        gio_file_clone.clone(),
                        win.imp().monochrome_switch.is_active(),
                        None,
                        Some(file_hash),
                        true,
                    )
                    .await;
                debug!("Done generating the small icon, now generating large one");
                _ = win
                    .save_file(
                        gio_file_temp.clone(),
                        win.imp().monochrome_switch.is_active(),
                        None,
                        Some(file_hash),
                        false,
                    )
                    .await;
                info!("Done image drag generation");
                std::fs::rename(
                    gio_file_temp.path().unwrap(),
                    gio_file_clone.path().unwrap(),
                )
                .log();
                debug!("File moved");
            }
        ));

        Some(gdk::ContentProvider::for_value(&glib::Value::from(
            &gio_file,
        )))
    }

    pub fn create_drag_file(&self, temp: bool) -> gio::File {
        let data_path = user_data_dir();
        debug!("data path: {:?}", data_path);
        let mut file_path = data_path.clone();
        let random_string = Uuid::now_v7();
        let generated_file_name = format!(
            "folder-{}{}.png",
            random_string,
            if temp { "-temp" } else { "" }
        );
        debug!("generated_file_name: {}", generated_file_name);
        file_path.push(generated_file_name.clone());
        debug!("generated file path: {:?}", file_path);
        let gio_file = gio::File::for_path(file_path);
        gio_file
    }

    pub fn drag_connect_cancel(&self, reason: gdk::DragCancelReason) -> bool {
        // let imp = self.imp();
        // let gio_file = imp
        //     .last_drag_n_drop_generated_name
        //     .borrow()
        //     .clone()
        //     .unwrap();
        self.image_save_sensitive(true);
        warn!(
            "Drag operation cancelled, removing file. Reason: {:?}\n(Currently disabled, treating as a succesful drag)",
            reason
        );
        // match gio_file.delete(None::<&Cancellable>) {
        //     Ok(_) => {
        //         debug!("Deletion succesfull!");
        //     }
        //     Err(e) => {
        //         warn!("Could not delete drag file, error: {:?}", e);
        //     }
        // };
        // imp.drag_cancelled.set(true);
        false
    }

    pub fn drag_connect_end(&self) {
        let imp = self.imp();
        imp.drag_active.set(false);
        debug!("drag end");
        if !imp.drag_cancelled.get() {
            // Drag event was not cancelled. I couldn't find a signal that fires only on a succseful drag
            debug!("succesful drag");
            let top_image = imp.top_image_file.lock().unwrap().clone().unwrap(); // Currently blocks

            match self.store_top_image_in_cache(&top_image) {
                Err(x) => {
                    show_error_popup(&self, "", true, Some(x));
                }
                _ => (),
            };
            self.drag_and_drop_regeneration_popup();
        }
        imp.drag_cancelled.set(false);
    }
}
