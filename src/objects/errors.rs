use crate::{GenError, GenResult, window::IconicWindow};
use adw::prelude::*;
use gettextrs::gettext;
use log::*;
use std::fmt::{Debug, Display};

pub fn show_error_popup<E>(
    window: &IconicWindow,
    message: &str,
    show: bool,
    error: Option<E>,
) -> Option<adw::AlertDialog>
where
    E: Display,
{
    const RESPONSE_OK: &str = "OK";
    let error_text: &str = &gettext("Error");
    let error_message = match message {
        "" => {
            if let Some(error_value) = &error {
                error_value.to_string()
            } else {
                "unknown error".to_string()
            }
        }
        _ => message.to_string(),
    };
    let dialog = adw::AlertDialog::builder()
        .heading(format!(
            "<span foreground=\"red\"><b>⚠ {error_text}</b></span>"
        ))
        .heading_use_markup(true)
        .body(error_message)
        .default_response(RESPONSE_OK)
        .close_response(RESPONSE_OK)
        .build();
    dialog.add_response(RESPONSE_OK, &gettext("OK"));
    match error {
        Some(ref x) => error!("An error has occured: \"{}\"", x.to_string()),
        None => error!("An error has occured: \"{}\"", message),
    };
    match show {
        true => {
            dialog.present(Some(window));
            None
        }
        false => Some(dialog),
    }
}

pub trait ErrorPopup<T, E> {
    fn _popup(&self, window: &IconicWindow) -> &Self;
    fn _popup_owned(self, window: &IconicWindow) -> Self;
    fn log(self) -> Self;
    fn map_err_to_str(self) -> GenResult<T>;
}

impl<T, E> ErrorPopup<T, E> for Result<T, E>
where
    E: Display,
{
    /// If `Err(T)` Log T to console
    fn log(self) -> Self {
        if let Err(error) = &self {
            warn!("Error: {}", error.to_string());
        }
        self
    }
    fn _popup(&self, window: &IconicWindow) -> &Self {
        if let Err(error) = self {
            show_error_popup(window, "", true, Some(error));
        }
        self
    }
    fn _popup_owned(self, window: &IconicWindow) -> Self {
        if let Err(error) = &self {
            show_error_popup(window, "", true, Some(error));
        }
        self
    }
    fn map_err_to_str(self) -> GenResult<T> {
        self.map_err(|err| (*Box::new(format!("{}", err.to_string()))).into())
    }
}

#[derive(Debug, thiserror::Error)]
enum OptionError {
    #[error("Unwrapped on a None value. (optional)reason: {0:?}")]
    NoneUnwrap(Option<&'static str>),
}

pub trait IntoResult<T> {
    fn into_result(self) -> Result<T, GenError>;
    fn into_reason_result(self, reason: &'static str) -> Result<T, GenError>;
}

impl<T> IntoResult<T> for Option<T> {
    fn into_result(self) -> Result<T, GenError> {
        match self {
            Some(value) => Ok(value),
            None => Err(Box::new(OptionError::NoneUnwrap(None))),
        }
    }
    fn into_reason_result(self, reason: &'static str) -> Result<T, GenError> {
        match self {
            Some(value) => Ok(value),
            None => Err(Box::new(OptionError::NoneUnwrap(Some(reason)))),
        }
    }
}
