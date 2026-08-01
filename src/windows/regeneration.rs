use crate::GenResult;
use crate::objects::errors::IntoResult;
use crate::objects::file::file::File;
use crate::objects::properties::{BottomImageType, FileProperties, PropertiesSource};
use crate::{IconicWindow, objects::errors::show_error_popup};

use crate::objects::properties::CustomRGB;
use adw::TimedAnimation;
use adw::{prelude::*, subclass::prelude::*};
use gettextrs::{gettext, ngettext};
use gio::glib::{self, user_cache_dir, user_data_dir};
use gio::prelude::SettingsExt;
use gtk::gdk::RGBA;
use gtk::gio;
use image::ImageFormat::Png;
use image::*;
use log::*;
use std::fs::{self, DirEntry};
use std::path::PathBuf;
use std::sync::Arc;

impl IconicWindow {
    pub fn check_if_regeneration_needed(&self) -> bool {
        let imp = self.imp();
        let previous_accent: String = imp.settings.string("previous-system-accent-color").into();
        let current_accent = self.get_accent_color();
        // error!("previous {previous_accent} current {current_accent}");
        let id = imp.regeneration_lock.get();
        imp.regeneration_lock.replace(id + 1);
        if previous_accent != current_accent && imp.settings.boolean("automatic-regeneration") {
            glib::spawn_future_local(glib::clone!(
                #[weak(rename_to = win)]
                self,
                async move {
                    match win.regenerate_icons().await {
                        Ok(_) => info!("Regeneration succesfull!"),
                        Err(x) => {
                            error!("{}", x.to_string());
                        }
                    };
                }
            ));
            return true;
        }
        false
    }

    pub fn store_top_image_in_cache(&self, file: &File) -> GenResult<()> {
        let imp = self.imp();
        if imp
            .file_properties
            .borrow()
            .bottom_image_type
            .is_regeneration_compatible()
            == None
        {
            info!("Current file does not use a compatible bottom image. no use caching the file");
            return Ok(());
        }
        //create folder inside cache, if it does not yet exist
        let cache_path = user_cache_dir().join("top_images");
        if !cache_path.exists() {
            debug!("Top icon cache dir does not yet exist, creating");
            fs::create_dir(&cache_path)?;
        }
        let file_name = file.hash;
        let mut file_path = cache_path.clone();
        file_path.push(file_name.to_string());
        debug!("File path: {:?}", file_path);
        debug!("File name: {:?}", file.filename);
        match file_path.exists() {
            true => {
                debug!("File already exists with name");
                return Ok(());
            }
            false => {
                debug!("File does not yet exist, creating");
                fs::File::create(&file_path)?;
            }
        };
        // Only if the orignal image path is present, and it has not been shrunk
        // Save the original, else save the generated dynamic image
        let new_file = gio::File::for_path(&file_path);
        let filestream = new_file.open_readwrite(gio::Cancellable::NONE)?;
        let test = filestream.output_stream();
        if let Some(original_file) = &file.files {
            if !file.image_resized {
                info!("Saving original image to cache");
                let buffer = original_file.load_bytes(gio::Cancellable::NONE)?;
                test.write_bytes(&buffer.0, gio::Cancellable::NONE)?;
                return Ok(());
            }
        }
        info!("Saving dynamic image to cache");
        file.image.save_with_format(file_path, ImageFormat::WebP)?;

        Ok(())
    }

    pub fn store_mask_in_cache(&self, main_filename: &str) -> GenResult<()> {
        let imp = self.imp();

        if let Some(mask) = imp.custom_mask.borrow().clone() {
            let cache_path = user_cache_dir().join("mask");
            if !cache_path.exists() {
                debug!("mask cache path not found, creating");
                fs::create_dir(&cache_path)?;
            }
            let mask_path = cache_path.clone().join(main_filename.to_owned());
            debug!("Saving custom mask");
            mask.save_with_format(mask_path, Png)?;
        } else {
            debug!("No custom mask")
        }
        Ok(())
    }

    // This function regenerates icon, it replaces all images that were dragged and dropped with ones of the correct system accent color.
    pub async fn regenerate_icons(&self) -> GenResult<()> {
        let imp = self.imp();
        let id = imp.regeneration_lock.get();
        // First set iconic as busy. By getting a Arc reference
        // I doubt this is the best approach, but Hey it works!
        let _iconic_busy = Arc::clone(&imp.app_busy);
        let data_path = user_data_dir();
        let mut incompatible_files_n: u32 = 0;
        let compatible_files =
            self.find_regeneratable_icons(data_path, &mut incompatible_files_n)?;

        // Stop if there are no files to regenerate
        match compatible_files.len() {
            0 => {
                imp.toast_overlay
                    .add_toast(adw::Toast::new(&gettext("Nothing to regenerate")));
                return Ok(());
            }
            _ => {
                imp.toast_overlay
                    .add_toast(adw::Toast::new(&gettext("Regenerating icons")));
            }
        }
        imp.regeneration_revealer.set_reveal_child(true);
        imp.regeneration_osd.set_fraction(0.0);
        imp.regeneration_osd_second.set_fraction(0.0);
        let files_n = compatible_files.len();
        let mut last_animation = None;
        let mut last_animation_second = None; // Second here means 2nd not sec
        let step_size = 1.0 / files_n as f64;
        let mut regeneration_errors = vec![];

        for file in compatible_files {
            // In the regeneration lock, a value is saved, if it has changed.
            // It means a new regeneration instance has started. So stop this one
            // This is done to prevent two instances from fighting if for example
            // The accent color is changed during regeneration
            if imp.regeneration_lock.get() != id {
                error!("Stopping regeneration");
                break;
            }
            let name = &file.1.file_name();
            match self
                .regenerate_and_save_single_icon(file.0, file.1, file.2)
                .await
            {
                Ok(_) => (),
                Err(error) => {
                    error!("Error while generating {:?}: {}", &name, &error.to_string());
                    // Everytime an error occurs. Push this to the errors list
                    regeneration_errors.push(error)
                }
            }
            // Update the progress bars
            // I need two as the animation needs to play on both welcome and main screen which is not possible
            // with only one animation. Unless i'm missing something
            last_animation = Some(self.progress_animation(
                step_size,
                last_animation,
                imp.regeneration_osd.clone(),
            ));
            last_animation_second = Some(self.progress_animation(
                step_size,
                last_animation_second,
                imp.regeneration_osd_second.clone(),
            ));
        }
        // If the errors list is not empty
        // Show a pop up with the ammount of errors
        if !regeneration_errors.is_empty() {
            show_error_popup(
                &self,
                &format!(
                    "{} {}",
                    regeneration_errors.len(),
                    &ngettext(
                        "file failed to regenerate\nview logs for more information",
                        "files failed to regenerate\nview logs for more information",
                        regeneration_errors.len() as u32
                    )
                ),
                true,
                None::<String>,
            );
        }
        imp.toast_overlay.add_toast(adw::Toast::new(&gettext(
            "Regeneration sucessful, restart nautilus",
        )));
        imp.regeneration_revealer.set_reveal_child(false);

        self.close_iconic_busy_popup();
        Ok(())
    }

    // This function regenerates a single compatible icon
    async fn regenerate_and_save_single_icon(
        &self,
        mut properties: FileProperties,
        file: DirEntry,
        property_source: PropertiesSource,
    ) -> GenResult<()> {
        let imp = self.imp();

        // Get path and filename of icon saved in datadir
        let file_path = file.path();

        let strict_mode_enabled = imp.settings.boolean("strict-regeneration");
        let ignore_custom_colored = imp.settings.boolean("ignore-custom");

        // Icons that are compatible for regeneration are only allowed to use default folder images.
        // So when regenerating icons, you need the folder which is the same color as the current accent color
        let (bottom_image_file, mask, custom_accent_color, custom_accent_color_hex) =
            match properties.bottom_image_type.clone() {
                BottomImageType::FolderSystem => (
                    self.get_bottom_icon_from_accent_color(None, strict_mode_enabled)
                        .await?,
                    // TODO No mask
                    // self.serve_mask(),
                    None,
                    None,
                    None,
                ),
                BottomImageType::Folder(color) if !properties.default || !strict_mode_enabled => (
                    self.get_bottom_icon_from_accent_color(
                        Some(color.clone()),
                        strict_mode_enabled,
                    )
                    .await?,
                    // TODO No maks
                    None,
                    Some(color),
                    None,
                ),
                BottomImageType::FolderCustom(foreground, background)
                    if !properties.default || (!strict_mode_enabled || !ignore_custom_colored) =>
                {
                    if strict_mode_enabled || ignore_custom_colored {
                        let folder_path = self
                            .create_custom_folder_color(&foreground, &background, true)
                            .await;
                        let mask_path = self.get_mask_path();
                        let image_file = gio::spawn_blocking(move || {
                            File::from_path(folder_path, 1024, 0, mask_path)
                                .map_err(|err| err.to_string())
                        })
                        .await
                        .unwrap()?;
                        (
                            image_file.image,
                            Some(image_file.image_mask),
                            None,
                            Some(background),
                        )
                    } else {
                        (
                            self.get_bottom_icon_from_accent_color(None, strict_mode_enabled)
                                .await?,
                            // TODO No mask
                            None,
                            None,
                            Some(background),
                        )
                    }
                }
                _ => return Ok(()),
            };
        info!("Generating image");

        // If strict mode is disabled and the image is not regenerated during strict mode. And the image is regenerated, is has te be regenerated to mark it as no longer default
        properties.default = if !strict_mode_enabled
            && properties.bottom_image_type.is_regeneration_compatible() == Some(false)
        {
            false
        } else {
            true
        };

        // Create the path where the top image of this file is located
        // The top image has the same name as the hash of that image
        let mut top_image_path = user_cache_dir().join("top_images");
        top_image_path.push(
            properties
                .top_image_hash
                .into_reason_result("Getting top image hash")?
                .to_string(),
        );
        let mask_path = self.get_mask_path();
        let top_image_file = gio::spawn_blocking(move || {
            File::from_path(top_image_path, 1024, 0, mask_path).map_err(|err| err.to_string())
        })
        .await
        .unwrap()?
        .image;
        // Create the top image
        let top_image = self.set_correct_monochrome_values_based_on_image_properties(
            &properties,
            top_image_file,
            custom_accent_color,
            custom_accent_color_hex,
            strict_mode_enabled,
        )?;

        // Using the generic generate_image function. The icon can faithfully be recreated
        // TODO mask is still not applied from everywhere
        let generated_image = self
            .generate_image(
                bottom_image_file,
                mask.unwrap_or_default(),
                top_image,
                imageops::FilterType::Gaussian,
                properties.x_val,
                properties.y_val,
                properties.zoom_val,
            )
            .await;
        info!("Saving image");
        let file_path_clone = file_path.clone();
        match gio::spawn_blocking(move || {
            generated_image.save_with_format(file_path, ImageFormat::Png)
        })
        .await
        .unwrap()
        {
            Ok(_) => {
                info!("Saving Succesful");
                if property_source == PropertiesSource::XMP {
                    self.add_image_metadata(file_path_clone, None, Some(properties))?
                }
            }
            Err(x) => error!("Saving failed: {:?}", x),
        };
        Ok(())
    }

    async fn get_bottom_icon_from_accent_color(
        &self,
        color: Option<String>,
        strict: bool,
    ) -> GenResult<DynamicImage> {
        let accent_color = match color {
            Some(color) if strict => color,
            _ => self.get_accent_color(),
        };

        // Icons that are compatible for regeneration are only allowed to use default folder images.
        // So when regenerating icons, you need the folder which is the same color as the current accent color
        let bottom_image_path = self.get_built_in_bottom_icon_path(&accent_color);
        let mask_path = self.get_mask_path();
        Ok(gio::spawn_blocking(move || {
            File::from_path(bottom_image_path, 1024, 0, mask_path).map_err(|err| err.to_string())
        })
        .await
        .unwrap()?
        .image)
    }

    // Search in the list of stored icons to see which ones are valid for regeneration
    fn find_regeneratable_icons(
        &self,
        dir: PathBuf,
        incompatible_files: &mut u32,
    ) -> GenResult<Vec<(FileProperties, fs::DirEntry, PropertiesSource)>> {
        let top_image_path = user_cache_dir().join("top_images");
        let mut regeneratable: Vec<(FileProperties, fs::DirEntry, PropertiesSource)> = vec![];
        // Walk the directory and loop over every file
        let files: fs::ReadDir = fs::read_dir(&dir)?;
        for file in files {
            let current_file = file?;
            let file_name = current_file.file_name();
            debug!("File found: {:?}", file_name);
            // Get the file properties of the file
            // Currently does not care what
            let properties = match FileProperties::get_file_properties(&current_file) {
                Ok(file_properties) => file_properties,
                Err(err) => {
                    *incompatible_files += 1;
                    warn!(
                        "file {:?} failed to be parsed. Err: {}",
                        file_name,
                        err.to_string()
                    );
                    continue;
                }
            };
            if properties
                .0
                .bottom_image_type
                .is_regeneration_compatible()
                .is_none()
            {
                *incompatible_files += 1;
                warn!(
                    "file {:?} is never compatible for be regeneration.",
                    file_name
                );
                continue;
            }
            let mut top_image_path_clone = top_image_path.clone();
            top_image_path_clone.push(properties.0.top_image_hash.unwrap_or_default().to_string());

            // If that top image does not exist, just mark it as not valid for regeneration
            if !top_image_path.exists() {
                warn!("Top image file not found");
                *incompatible_files += 1;
                continue;
            }
            regeneratable.push((properties.0, current_file, properties.1));
        }
        Ok(regeneratable)
    }

    // Create the top image based on the properties of the to-be regenerated icon
    // I am really bad at function names
    fn set_correct_monochrome_values_based_on_image_properties(
        &self,
        properties: &FileProperties,
        top_image: DynamicImage,
        accent_color: Option<String>,
        rgb_string_color: Option<String>,
        strict: bool,
    ) -> GenResult<DynamicImage> {
        let color = match properties.monochrome_default {
            false => RGBA::new(
                properties.monochrome_color.unwrap_or_default().0 as f32 / 255.0,
                properties.monochrome_color.unwrap_or_default().1 as f32 / 255.0,
                properties.monochrome_color.unwrap_or_default().2 as f32 / 255.0,
                1.0,
            ),
            true if rgb_string_color.is_some() && strict => {
                RGBA::from_hex(rgb_string_color.unwrap_or_default())
            }
            true => self.current_accent_rgba(if strict { accent_color } else { None })?,
        };
        match properties.monochrome_toggle {
            true => Ok(self.to_monochrome(
                top_image,
                properties.monochrome_threshold_val,
                color,
                Some(properties.monochrome_invert),
            )),
            _ => Ok(top_image),
        }
    }

    fn current_accent_rgba(&self, accent_color: Option<String>) -> GenResult<RGBA> {
        let imp = self.imp();
        let accent_color = match accent_color {
            Some(color) => color,
            None => self.get_accent_color(),
        };
        Ok(imp
            .default_colors
            .borrow()
            .get(&accent_color)
            .into_result()?
            .clone())
    }

    fn progress_animation(
        &self,
        step_size: f64,
        previous_animation: Option<TimedAnimation>,
        progress_bar: gtk::ProgressBar,
    ) -> TimedAnimation {
        let target = adw::PropertyAnimationTarget::new(&progress_bar, "fraction");
        let _ = previous_animation.is_some_and(|animation| {
            animation.skip();
            false
        });
        let animation = adw::TimedAnimation::builder()
            .target(&target)
            .widget(&progress_bar)
            .value_from(progress_bar.fraction())
            .value_to(progress_bar.fraction() + step_size)
            .duration(200)
            .easing(adw::Easing::EaseInOutCubic)
            .build();
        animation.play();
        animation
    }
}
