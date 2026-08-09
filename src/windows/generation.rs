use adw::{prelude::*, subclass::prelude::*};
use gtk::gdk;
use image::*;

use crate::IconicWindow;

impl IconicWindow {
    pub async fn render_to_screen(&self) {
        let imp = self.imp();
        let base_file = imp
            .bottom_image_file
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone();
        let base = base_file.thumbnail;
        let base_mask = base_file.thumbnail_mask;
        let mut top_image = imp
            .top_image_file
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .thumbnail
            .clone();
        if imp.monochrome_switch.state() {
            top_image = self.to_monochrome(
                top_image,
                imp.threshold_scale.value() as u8,
                imp.monochrome_color.rgba(),
                None,
            );
        }
        let generated_image = &self
            .generate_image(
                base,
                self.serve_mask(base_mask),
                top_image,
                imageops::FilterType::Nearest,
                imp.x_scale.value(),
                imp.y_scale.value(),
                imp.size.value(),
            )
            .await;
        self.image_save_sensitive(true);
        let texture = self.dynamic_image_to_texture(&generated_image);
        imp.image_view.set_paintable(&texture);
        imp.image_view.queue_draw();
    }

    // This function creates the monochrome image everytime an update happens to the screen
    // That is so inefficient, so I need to rethink that
    // TODO rethink the monochrome implementation
    pub fn to_monochrome(
        &self,
        image: DynamicImage,
        threshold: u8,
        color: gdk::RGBA,
        invert: Option<bool>,
    ) -> DynamicImage {
        // Convert the image to RGBA8
        let rgba_img = image.to_rgba8();
        // Define a threshold value
        let threshold = threshold; // Adjust the threshold value as needed

        // Create a new image buffer for the monochrome image
        let mut mono_img: RgbaImage = ImageBuffer::new(rgba_img.width(), rgba_img.height());
        let switch_state = match invert {
            Some(invert_value) => invert_value,
            None => self.imp().monochrome_invert.is_active(),
        };
        // Apply the threshold to create a black and white image, keeping the alpha channel
        for (x, y, pixel) in rgba_img.enumerate_pixels() {
            let rgba = pixel.0;
            let luma = 0.299 * rgba[0] as f32 + 0.587 * rgba[1] as f32 + 0.114 * rgba[2] as f32;
            let threshold_reached = luma >= threshold as f32 && rgba[3] > 0;
            let mono_pixel = match (threshold_reached, switch_state) {
                (false, false) | (true, true) => Rgba([0u8, 0u8, 0u8, 0u8]), // Black
                _ => self.create_colored_pixel(color, rgba[3]), // White with original alpha
            };
            mono_img.put_pixel(x, y, mono_pixel);
        }

        // Convert the monochrome RgbaImage to DynamicImage
        DynamicImage::ImageRgba8(mono_img)
    }

    fn create_colored_pixel(&self, color: gdk::RGBA, original_alpha: u8) -> Rgba<u8> {
        Rgba([
            (color.red() * 255.0) as u8,
            (color.green() * 255.0) as u8,
            (color.blue() * 255.0) as u8,
            original_alpha,
        ])
    }

    pub async fn generate_image(
        &self,
        base_image: DynamicImage,
        base_mask: Option<DynamicImage>,
        top_image: DynamicImage,
        filter: imageops::FilterType,
        x_scale_value: f64,
        y_scale_value: f64,
        scale: f64,
    ) -> DynamicImage {
        let imp = self.imp();
        let empty_image = DynamicImage::new_rgba8(base_image.width(), base_image.height());
        let coordinates = ((x_scale_value + 50.0) as i64, (y_scale_value + 50.0) as i64);
        let texture = gio::spawn_blocking(move || {
            let mut empty_base = empty_image;
            let mut base = base_image;
            let top = top_image;
            let base_dimension: (i64, i64) = (
                (empty_base.dimensions().0).into(),
                (empty_base.dimensions().1).into(),
            );
            let top = IconicWindow::resize_top_image(top, empty_base.dimensions(), scale, filter);
            let top_dimension: (i64, i64) = (
                (top.dimensions().0 / 2).into(),
                (top.dimensions().1 / 2).into(),
            );
            let final_coordinates: (i64, i64) = (
                ((base_dimension.0 * coordinates.0) / 100) - top_dimension.0,
                ((base_dimension.1 * coordinates.1) / 100) - top_dimension.1,
            );
            imageops::overlay(
                &mut empty_base,
                &top,
                final_coordinates.0.into(),
                final_coordinates.1.into(),
            );
            empty_base = if let Some(mask) = base_mask {
                IconicWindow::apply_mask_to_top_image(empty_base, mask)
            } else {
                empty_base
            };
            imageops::overlay(&mut base, &empty_base, 0, 0);
            base
        })
        .await
        .unwrap();
        imp.generated_image.replace(Some(texture.clone()));
        texture
    }

    pub fn resize_top_image(
        image: DynamicImage,
        dimensions: (u32, u32),
        slider_position: f64,
        filter: imageops::FilterType,
    ) -> DynamicImage {
        let width: f64 = dimensions.0 as f64;
        let height: f64 = dimensions.1 as f64;
        let scale_factor: f64 = (slider_position + 10.0) / 10.0;
        let new_width: u32 = (width / scale_factor) as u32;
        let new_height: u32 = (height / scale_factor) as u32;
        image.resize(new_width, new_height, filter)
    }
}
