use std::path::PathBuf;

use image::{DynamicImage, imageops};
use log::*;

use crate::{GenResult, objects::file::file::File};

type MainMask = DynamicImage;
type ThumbnailMask = DynamicImage;

/// A way to specify at what path the mask is located.
/// Where if it is set to automatic, the path gets generated automatically
#[derive(Debug, PartialEq)]
pub enum MaskOption {
    Custom(PathBuf),
    Automatic,
    Disabled,
}

impl super::file::File {
    pub fn auto_generate_mask(image: &DynamicImage) -> DynamicImage {
        let bottom_image_pixels = image.to_rgba8();
        let mut mask_pixels = bottom_image_pixels.clone();

        for (x, y, pixel) in bottom_image_pixels.enumerate_pixels() {
            let mask_pixel = mask_pixels.get_pixel_mut(x, y);
            *mask_pixel = image::Rgba([pixel[3], pixel[3], pixel[3], 255u8]);
        }
        DynamicImage::ImageRgba8(mask_pixels)
    }

    // Load the mask dynamicimages resized to requested size
    pub(super) fn get_masks(
        main_size: u32,
        thumbnail_size: u32,
        image: &DynamicImage,
        mask_path: MaskOption,
    ) -> GenResult<(Option<MainMask>, Option<ThumbnailMask>)> {
        if mask_path == MaskOption::Disabled {
            return Ok((None, None));
        }
        let image_mask = if let MaskOption::Custom(path) = mask_path {
            Self::load_file(&gio::File::for_path(path), main_size)
                .map_err(|e| format!("Failed to load path: {}", e.to_string()))?
                .0
        } else {
            Self::auto_generate_mask(&image)
        };
        let thumbnail_mask = if thumbnail_size > 0 {
            Some(image_mask.clone().resize_exact(
                thumbnail_size,
                thumbnail_size,
                imageops::FilterType::Nearest,
            ))
        } else {
            None
        };
        Ok((Some(image_mask), thumbnail_mask))
    }

    pub fn create_mask_dynamicimage(
        mask_path: MaskOption,
        bottom_image: &DynamicImage,
    ) -> Option<DynamicImage> {
        let size = bottom_image.height();
        let mut auto_generated_mask = DynamicImage::new_rgba8(size, size);
        if mask_path == MaskOption::Automatic {
            auto_generated_mask = File::auto_generate_mask(bottom_image); // The auto generated mask only needs to be filled in if it actually needs to be created
        }
        match mask_path {
            MaskOption::Disabled => None,
            _ => File::get_masks(size, 0, &auto_generated_mask, mask_path)
                .map(|x| {
                    warn!("Failed to load mask, automatically generating");
                    x.0.unwrap_or_else(|| File::auto_generate_mask(bottom_image))
                })
                .ok(),
        }
    }
}
