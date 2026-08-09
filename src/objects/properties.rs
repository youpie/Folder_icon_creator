use std::{fs::DirEntry, path::PathBuf};

use adw::subclass::prelude::ObjectSubclassIsExt;
use gio::{Settings, prelude::SettingsExt};
use gtk::gdk;
use gtk::prelude::RangeExt;
use hex::FromHex;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use strum::Display;
use thiserror::Error;
use xmp_toolkit::{XmpMeta, XmpValue, xmp_ns};

use crate::{GenResult, objects::errors::IntoResult, window::IconicWindow};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PropertiesSource {
    XMP,
    Filename,
}

#[derive(Debug, Clone, Default)]
pub struct FileProperties {
    pub bottom_image_type: BottomImageType,
    pub top_image_hash: Option<u64>,
    pub x_val: f64,
    pub y_val: f64,
    pub zoom_val: f64,
    pub monochrome_toggle: bool,
    pub monochrome_invert: bool,
    pub monochrome_default: bool,
    pub monochrome_color: Option<(u8, u8, u8)>,
    pub monochrome_threshold_val: u8,
    pub mask: MaskType,
    pub desktop: DesktopEnvironment,
    /// If the values above are still equal with the generated image. False if for example, the image was regenerated
    pub default: bool,
}

impl FileProperties {
    // File properties are not (yet) automatically copied from the global file properties.
    // Remember to manually add that link in this function
    pub fn new(
        imp: &IconicWindow,
        top_image_hash: Option<u64>,
        default_monochrome_color: gdk::RGBA,
    ) -> Self {
        let imp = imp.imp();
        let global_file_properties = imp.file_properties.borrow().clone();
        let x_val = imp.x_scale.value();
        let y_val = imp.y_scale.value();
        let zoom_val = imp.size.value();
        let monochrome_toggle = imp.monochrome_switch.is_active();
        let monochrome_color = if monochrome_toggle {
            let red = (imp.monochrome_color.rgba().red() * 255.0) as u8;
            let green = (imp.monochrome_color.rgba().green() * 255.0) as u8;
            let blue = (imp.monochrome_color.rgba().blue() * 255.0) as u8;
            Some((red, green, blue))
        } else {
            None
        };
        let monochrome_default = default_monochrome_color == imp.monochrome_color.rgba();
        let monochrome_threshold_val = imp.threshold_scale.value() as u8;
        let monochrome_invert = imp.monochrome_invert.is_active();
        let mask = global_file_properties.mask;
        let desktop = DesktopEnvironment::default(); // TODO custom desktop logic
        let bottom_image_type = global_file_properties.bottom_image_type;
        Self {
            bottom_image_type,
            top_image_hash,
            x_val,
            y_val,
            zoom_val,
            monochrome_color,
            monochrome_default,
            monochrome_invert,
            monochrome_threshold_val,
            monochrome_toggle,
            mask,
            desktop,
            default: true,
        }
    }

    pub fn get_file_properties(file: &DirEntry) -> GenResult<(Self, PropertiesSource)> {
        if let Ok(xmp_data) = XmpMeta::from_file(file.path()) {
            info!("loading image from XMP");
            Ok((Self::from_xmp_data(xmp_data)?, PropertiesSource::XMP))
        } else {
            info!("loading image from Filename");
            let file_name = file.file_name();
            Ok((
                Self::from_filename(file_name.to_string_lossy().to_string())?,
                PropertiesSource::Filename,
            ))
        }
    }
    // Load all properties from the filename
    fn from_filename(filename: String) -> GenResult<Self> {
        // I assume one day ill forget that i do this and get confused
        let filename = filename.replace(".png", "");
        debug!("parsing: {}", filename);
        let file_properties = filename.split("-");

        let properties_list: Vec<&str> = file_properties.into_iter().collect();

        // If filename is not folder_new it is not compatible
        match properties_list[FilenameProperty::FileName as usize] {
            "folder_new" => (),
            _ => return Err(Box::new(PropertiesError::Incompatible)),
        }
        let x_val: f64 = properties_list[FilenameProperty::XScale as usize].parse()?;
        let y_val: f64 = properties_list[FilenameProperty::YScale as usize].parse()?;
        let zoom_val: f64 = properties_list[FilenameProperty::ZoomVal as usize].parse()?;
        let monochrome_toggle: bool = properties_list
            [FilenameProperty::MonochromeSelected as usize]
            .parse::<u8>()?
            .to_bool();
        let monochrome_invert: bool = properties_list
            [FilenameProperty::MonochromeInverted as usize]
            .parse::<u8>()?
            .to_bool();
        let monochrome_default: bool =
            properties_list[FilenameProperty::DefaultMonochromeColor as usize].parse()?;
        let monochrome_threshold_val: u8 =
            properties_list[FilenameProperty::MonochromeThreshold as usize].parse()?;
        let monochrome_color = if !monochrome_default {
            let mut rgb: (u8, u8, u8) = (0, 0, 0);
            rgb.0 = (properties_list[FilenameProperty::MonochromeRed as usize].parse::<f32>()?
                * 255.0) as u8;
            rgb.1 = (properties_list[FilenameProperty::MonochromeGreen as usize].parse::<f32>()?
                * 255.0) as u8;
            rgb.2 = (properties_list[FilenameProperty::MonochromeBlue as usize].parse::<f32>()?
                * 255.0) as u8;
            Some(rgb)
        } else {
            None
        };
        let top_image_hash: Option<u64> =
            Some(properties_list[FilenameProperty::Hash as usize].parse()?);
        let bottom_image_type = match properties_list[FilenameProperty::DefaultBottomImage as usize]
            .parse::<u8>()?
            .to_bool()
        {
            true => BottomImageType::FolderSystem,
            false => BottomImageType::Unknown,
        };

        let mask = MaskType::Automatic;
        let desktop = DesktopEnvironment::default();

        Ok(Self {
            x_val,
            y_val,
            zoom_val,
            monochrome_color,
            monochrome_default,
            monochrome_invert,
            monochrome_threshold_val,
            monochrome_toggle,
            top_image_hash,
            bottom_image_type,
            mask,
            desktop,
            default: true,
        })
    }
    fn from_xmp_data(xmp_data: XmpMeta) -> GenResult<Self> {
        let x_val: f64 = xmp_data
            .property(xmp_ns::XMP, "x_val")
            .into_reason_result("XMP X-val")?
            .value
            .parse()?;
        let y_val: f64 = xmp_data
            .property(xmp_ns::XMP, "y_val")
            .into_reason_result("XMP Y-val")?
            .value
            .parse()?;
        let zoom_val: f64 = xmp_data
            .property(xmp_ns::XMP, "zoom_val")
            .into_reason_result("XMP Zoom-val")?
            .value
            .parse()?;
        let monochrome_toggle: bool = xmp_data
            .property(xmp_ns::XMP, "monochrome_toggle")
            .into_reason_result("XMP monochrome_toggle")?
            .value
            .parse()?;
        let monochrome_color: Option<(u8, u8, u8)> = if monochrome_toggle {
            let red: u8 = xmp_data
                .property(xmp_ns::XMP, "monochrome_red")
                .into_reason_result("XMP monochrome_red")?
                .value
                .parse()?;
            let green: u8 = xmp_data
                .property(xmp_ns::XMP, "monochrome_green")
                .into_reason_result("XMP monochrome_green")?
                .value
                .parse()?;
            let blue: u8 = xmp_data
                .property(xmp_ns::XMP, "monochrome_blue")
                .into_reason_result("XMP monochrome_blue")?
                .value
                .parse()?;
            Some((red, green, blue))
        } else {
            None
        };
        let monochrome_default: bool = xmp_data
            .property(xmp_ns::XMP, "monochrome_default")
            .into_reason_result("XMP monochrome_default")?
            .value
            .parse()?;
        let monochrome_invert: bool = xmp_data
            .property(xmp_ns::XMP, "monochrome_invert")
            .into_reason_result("XMP monochrome_invert")?
            .value
            .parse()?;
        let monochrome_threshold_val: u8 = xmp_data
            .property(xmp_ns::XMP, "monochrome_threshold")
            .into_reason_result("XMP monochrome_threshold")?
            .value
            .parse()?;
        let top_image_hash: Option<u64> = xmp_data
            .property(xmp_ns::XMP, "top_image_hash")
            .and_then(|value| Some(value.value.parse().unwrap_or_default()));
        let bottom_image_type: BottomImageType = serde_json::from_str(
            &xmp_data
                .property(xmp_ns::XMP, "bottom_image_type")
                .into_reason_result("XMP bottom_image_type")?
                .value,
        )?;
        let mask: MaskType = serde_json::from_str(
            &xmp_data
                .property(xmp_ns::XMP, "mask")
                .unwrap_or(XmpValue::new("Automatic".to_owned()))
                .value,
        )?;
        let desktop: DesktopEnvironment = serde_json::from_str(
            &xmp_data
                .property(xmp_ns::XMP, "desktop")
                .unwrap_or(XmpValue::new("Gnome".to_owned()))
                .value,
        )?;
        let default: bool = xmp_data
            .property(xmp_ns::XMP, "default")
            .unwrap_or(XmpValue::new("true".to_owned()))
            .value
            .parse()?;
        Ok(Self {
            x_val,
            y_val,
            zoom_val,
            monochrome_color,
            monochrome_default,
            monochrome_invert,
            monochrome_threshold_val,
            monochrome_toggle,
            top_image_hash,
            bottom_image_type,
            mask,
            desktop,
            default,
        })
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Display)]
pub enum DesktopEnvironment {
    #[default]
    Gnome,
    Kde,
    Plasma,
}

#[derive(Debug, Error)]
pub enum PropertiesError {
    #[error("The provided filename is not compatible")]
    Incompatible,
}

type Background = String;
type Foreground = String;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum BottomImageType {
    #[default]
    Unknown,
    FolderSystem,
    Folder(String),
    FolderCustom(Foreground, Background),
    Custom(PathBuf),
}

impl BottomImageType {
    // Whether this image is able to be regenerate with strict mode enabled
    // By returning none, the image is not at all compatible for regeneration
    pub fn is_regeneration_compatible(&self) -> Option<bool> {
        match self {
            Self::FolderSystem => Some(true),
            Self::FolderCustom(_, _) => Some(false),
            Self::Folder(value) if value != "Unknown" => Some(false),
            _ => None,
        }
    }

    // Get the base bottom image state
    pub fn get_standard(window: &IconicWindow) -> Self {
        let imp = window.imp();
        if imp.settings.boolean("manual-bottom-image-selection") {
            let cache_file_name: String = imp.settings.string("folder-cache-name").into();
            let path = window.check_chache_icon(&cache_file_name);
            BottomImageType::Custom(path)
        } else if imp.settings.string("selected-accent-color") == "Custom" {
            let custom_primary_color: String = imp.settings.string("primary-folder-color").into();
            let custom_secondary_color: String =
                imp.settings.string("secondary-folder-color").into();
            BottomImageType::FolderCustom(custom_primary_color, custom_secondary_color)
        } else {
            let set_folder_color: String = imp.settings.string("selected-accent-color").into();
            match set_folder_color.as_str() {
                "None" => BottomImageType::FolderSystem,
                _ => BottomImageType::Folder(set_folder_color),
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum MaskType {
    #[default]
    Disabled,
    Automatic,
    Custom,
}

impl MaskType {
    pub fn from_settings(settings: &Settings) -> Self {
        match settings.boolean("mask-enabled") {
            false => Self::Disabled,
            true => Self::Automatic,
        }
    }
}

// The properties of the to be regenerated file, are stored in the filename
// To get the properties, split the filename at the '-'.
// This enum provides a more visual method of knowing at what index each property is..
pub enum FilenameProperty {
    FileName = 0,
    DefaultBottomImage,
    XScale,
    YScale,
    ZoomVal,
    MonochromeSelected,
    MonochromeThreshold,
    MonochromeRed,
    MonochromeGreen,
    MonochromeBlue,
    MonochromeInverted,
    DefaultMonochromeColor,
    Hash,
}

// Why must this exist!!!!
trait ToBool {
    fn to_bool(&self) -> bool;
}

impl ToBool for u8 {
    fn to_bool(&self) -> bool {
        *self != 0
    }
}

pub trait CustomRGB {
    fn from_rgb(r: u8, g: u8, b: u8) -> Self;
    fn to_hex(&self) -> String;
    fn from_hex(hex: String) -> Self;
}

impl CustomRGB for gdk::RGBA {
    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let r_float = (1.0 / 255.0 * r as f64) as f32;
        let g_float = (1.0 / 255.0 * g as f64) as f32;
        let b_float = (1.0 / 255.0 * b as f64) as f32;
        gdk::RGBA::new(r_float, g_float, b_float, 1.0)
    }

    fn from_hex(hex: String) -> Self {
        let decoded = <[u8; 3]>::from_hex(hex).unwrap_or([255, 255, 255]);
        Self::from_rgb(decoded[0], decoded[1], decoded[2])
    }

    fn to_hex(&self) -> String {
        let red = format!("{:02X?}", (self.red() * 255.0) as u8);
        let green = format!("{:02X?}", (self.green() * 255.0) as u8);
        let blue = format!("{:02X?}", (self.blue() * 255.0) as u8);

        let hex = format!("{}{}{}", red, green, blue);
        debug!("{}", &hex);
        hex
    }
}
