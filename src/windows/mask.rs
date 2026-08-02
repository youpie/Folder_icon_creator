use std::path::PathBuf;

use adw::subclass::prelude::*;

use gio::glib::user_cache_dir;
use image::{DynamicImage, GenericImageView};
use log::*;
use uuid::Uuid;

use crate::{
    GenResult,
    objects::{
        file::{file::File, mask::MaskOption},
        properties::{FileProperties, MaskType},
    },
    window::IconicWindow,
};

/*
    TODO please add a small explanation of the mask logic,
    how do I keep building new features which are so complicatedly built that forget how they work :/
*/

impl IconicWindow {
    /// Pass `None` to properties if you want the global properties
    pub fn get_mask_path(&self, properties: Option<FileProperties>) -> MaskOption {
        let imp = self.imp();
        let properties = properties.unwrap_or(imp.file_properties.borrow().clone());
        let mask = properties
            .bottom_image_type
            .is_regeneration_compatible()
            .map(|_| self.load_builtin_mask_path());
        match mask {
            Some(mask_path) => MaskOption::Custom(mask_path),
            None => MaskOption::Automatic,
        }
    }

    /// Serve the correct mask based on settings
    /// Pass the mask image from the `Iconic::File`.
    pub fn serve_mask(&self, mask: Option<DynamicImage>) -> Option<DynamicImage> {
        let imp = self.imp();
        if let Some(mask) = mask {
            let mask_setting = imp.file_properties.borrow().mask.clone();
            match mask_setting {
                MaskType::Automatic => Some(mask),
                MaskType::Disabled => None,
                MaskType::Custom(_) => {
                    let custom_mask = imp.custom_mask.borrow().clone();
                    if let Some(custom_mask) = custom_mask  // Custom mask must be the same dimension as normal mask for correct function
                        && custom_mask.dimensions() != mask.dimensions()
                    {
                        Some(custom_mask.resize(
                            mask.width(),
                            mask.height(),
                            image::imageops::FilterType::Nearest,
                        ))
                    } else {
                        None
                    }
                }
            }
        } else {
            // the mask from the iconic file can be empty (if top image)
            // This is wrong but no mask will be applied
            warn!("Empty Mask provided (likely programming error)");
            None
        }
    }

    fn load_builtin_mask_path(&self) -> PathBuf {
        let mut path = self.get_built_in_bottom_icon_path("None");
        path.set_file_name("mask.svg");
        path
    }

    pub fn apply_mask_to_top_image(top_image: DynamicImage, mask: DynamicImage) -> DynamicImage {
        let mask_pixels = mask.to_rgba8();
        let mut top_image_pixels = top_image.to_rgba8();
        for (x, y, pixel) in top_image_pixels.enumerate_pixels_mut() {
            let mask_pixel = mask_pixels.get_pixel(x, y);
            let mask_pixel_luminance = (0.299 * mask_pixel[0] as f32
                + 0.587 * mask_pixel[1] as f32
                + 0.114 * mask_pixel[2] as f32) as u8;
            if pixel[3] > mask_pixel_luminance {
                pixel[3] = mask_pixel_luminance;
            };
        }

        DynamicImage::ImageRgba8(top_image_pixels)
    }

    pub async fn choose_custom_mask(&self) -> GenResult<()> {
        let imp = self.imp();
        let file_option = self.open_file_chooser().await;
        if let Some(file) = file_option {
            let file_name = Uuid::now_v7().to_string();

            let mut properties = imp.file_properties.borrow().clone();
            properties.mask = MaskType::Custom(file_name);
            imp.file_properties.replace(properties);

            let image = File::load_file(&file, 1024)?;
            imp.custom_mask.replace(Some(image.0));
        }
        Ok(())
    }

    /// Load mask for regeneration
    pub fn load_mask(
        &self,
        properties: &FileProperties,
        file_name: &str,
        bottom_image: &DynamicImage,
    ) -> Option<DynamicImage> {
        let mask_type = properties.mask.clone();
        debug!("mask type for image {file_name} is {mask_type:?}");
        let mask_path = match mask_type {
            MaskType::Automatic => self.get_mask_path(Some(properties.clone())),
            MaskType::Disabled => MaskOption::Disabled,
            MaskType::Custom(_) => {
                MaskOption::Custom(user_cache_dir().join("masks").join(file_name))
            }
        };
        File::create_mask_dynamicimage(mask_path, bottom_image)
    }
}
