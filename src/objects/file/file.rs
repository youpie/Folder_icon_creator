use adw::prelude::FileExt;
use gio::{Cancellable, FileInfo, FileQueryInfoFlags};
use gtk::gio;
use image::imageops::FilterType::Nearest;
use image::*;
use log::*;
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{Options, Transform, Tree};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use crate::GenResult;
use crate::objects::errors::IntoResult;
use crate::objects::file::mask::MaskOption;

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub files: Option<gio::File>,
    pub path: PathBuf,
    pub filename: String,
    pub extension: String,
    pub image: DynamicImage,
    pub image_mask: Option<DynamicImage>,
    pub image_resized: bool,
    pub thumbnail: DynamicImage,
    pub thumbnail_mask: Option<DynamicImage>,
    pub hash: u64,
}

impl File {
    pub fn path_str(&self) -> String {
        self.path.clone().into_os_string().into_string().unwrap()
    }

    pub fn new(
        image_file: gio::File,
        size: u32,
        thumbnail_size: u32,
        mask_path: MaskOption,
    ) -> GenResult<Self> {
        let (mut image, file_info) = Self::load_file(&image_file, size)?;
        let image_path = image_file
            .path()
            .into_reason_result("Can't get file path")?;
        let filename = PathBuf::from(file_info.name().into_os_string());
        let file_name = filename
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or("Unknown".to_owned());
        let file_extension = filename
            .extension()
            .map(|extension| extension.to_string_lossy().to_string())
            .unwrap_or("png".to_owned());

        let hash = Self::create_hash(&image);
        debug!("hash of created file: {}", hash);

        // Resize the image if the file is larger than the set size
        // Reduces cache file size, but will take even longer to load image
        let image_resized = if image.width() > size as u32 || image.height() > size as u32 {
            image = image.resize(size as u32, size as u32, imageops::FilterType::Nearest);
            true
        } else {
            false
        };

        // Thumbnail logic
        let thumbnail = if thumbnail_size > 0 {
            image.resize(
                thumbnail_size,
                thumbnail_size,
                imageops::FilterType::CatmullRom,
            )
        } else {
            DynamicImage::new_rgba8(0, 0)
        };

        // Mask logic
        let (image_mask, thumbnail_mask) =
            Self::get_masks(size, thumbnail_size, &image, &mask_path)?;

        Ok(Self {
            files: Some(image_file),
            path: image_path,
            extension: file_extension,
            filename: file_name,
            image,
            image_mask,
            image_resized,
            thumbnail,
            thumbnail_mask,
            hash,
        })
    }

    pub fn load_file(file: &gio::File, size: u32) -> GenResult<(DynamicImage, FileInfo)> {
        let path = file.path().into_reason_result("Can't get file path")?;
        let file_info =
            file.query_info("standard::", FileQueryInfoFlags::NONE, Cancellable::NONE)?;
        let mime_type = file_info.content_type();
        let image = if mime_type == Some("image/svg+xml".into()) {
            Self::load_svg(path, size)?
        } else {
            ImageReader::open(path)?.with_guessed_format()?.decode()?
        };
        Ok((image, file_info))
    }

    pub fn from_path(
        path: impl AsRef<std::path::Path>,
        size: u32,
        thumbnail_size: u32,
        mask_path: MaskOption,
    ) -> GenResult<Self> {
        let file = gio::File::for_path(path);
        Self::new(file, size, thumbnail_size, mask_path)
    }

    pub fn from_image(
        image: DynamicImage,
        thumbnail_size: u32,
        max_size: Option<u32>,
        filename: &str,
    ) -> Self {
        let mut image = image;
        let thumbnail = image.clone().resize(
            thumbnail_size,
            thumbnail_size,
            imageops::FilterType::Nearest,
        );
        let hash = Self::create_hash(&image);
        let mut image_resized = false;
        // Resize the image if the file is larger than the set size
        // Reduces cache file size, but will take even longer to load image
        if let Some(size) = max_size {
            if image.width() > size || image.height() > size {
                image_resized = true;
                image = image.resize(size, size, imageops::FilterType::Nearest);
            }
        }

        let image_mask = Self::auto_generate_mask(&image);
        let thumbnail_mask = Some(image_mask.resize(thumbnail_size, thumbnail_size, Nearest));
        let image_mask = Some(image_mask);

        Self {
            files: None,
            path: "".into(),
            extension: ".dynamic".to_string(),
            filename: filename.to_string(),
            hash,
            image,
            image_mask,
            thumbnail,
            thumbnail_mask,
            image_resized,
        }
    }

    fn load_svg(path: PathBuf, size: u32) -> GenResult<DynamicImage> {
        // Load the SVG file content
        let svg_data = match fs::read(path) {
            Ok(x) => x,
            Err(_) => fs::read("/usr/share/icons/Adwaita/scalable/places/folder.svg")?,
        };
        // Create an SVG tree
        let opt = Options::default();
        let rtree = Tree::from_data(&svg_data, &opt)?;

        // Specify the output dimensions (you can adjust these as needed)
        let width = rtree.size().width();
        let height = rtree.size().height();

        // Calculate the scale factor
        let scale_x = size as f32 / width;
        let scale_y = size as f32 / height;
        let scale = scale_x.min(scale_y); // Maintain aspect ratio

        // Create a Pixmap to render into
        let mut pixmap = Pixmap::new(size, size).unwrap();

        // Render the SVG tree to the Pixmap
        resvg::render(
            &rtree,
            Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        Ok(Self::pixmap_to_image(pixmap))
    }

    // Adds hash of file to file name
    fn create_hash(image: &DynamicImage) -> u64 {
        let mut hasher = DefaultHasher::new();
        let _ = image.as_bytes().hash(&mut hasher);
        hasher.finish()
    }

    fn pixmap_to_image(pixmap: Pixmap) -> DynamicImage {
        // Create an empty RgbaImage with the same dimensions as the Pixmap.
        let pixmap_clone = pixmap.clone();
        let mut img = RgbaImage::new(pixmap_clone.width(), pixmap.height());

        // Iterate over each pixel in the Pixmap.
        for y in 0..pixmap_clone.height() {
            for x in 0..pixmap_clone.width() {
                // Get the pixel at (x, y). `pixel` returns an Option, we unwrap it safely because we know (x, y) is valid.
                if let Some(pixel) = pixmap_clone.pixel(x, y) {
                    // Copy the pixel's RGBA data into the RgbaImage.
                    img.put_pixel(
                        x,
                        y,
                        Rgba([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]),
                    );
                }
            }
        }

        // Convert the RgbaImage to a DynamicImage.
        DynamicImage::ImageRgba8(img)
    }
}
