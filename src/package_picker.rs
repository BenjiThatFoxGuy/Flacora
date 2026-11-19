//! Native package selection for Rune versions whose file dialog picks one file.

use crate::store::PACKAGE;
use rune::raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use std::path::PathBuf;

pub const TITLE: &str = "Select Extension Packages";
pub const FILTER: &str = "Extension Packages (*.fext)";
pub const EXTENSIONS: [&str; 1] = [PACKAGE];

struct Parent<'a>(WindowHandle<'a>);

impl HasWindowHandle for Parent<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(self.0)
    }
}

impl HasDisplayHandle for Parent<'_> {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

pub fn open(window: &rune::WindowHandle) -> Result<impl FnOnce() -> Vec<PathBuf> + Send + use<>, String> {
    let selection = window.with_raw_handle(|handle| {
        rfd::AsyncFileDialog::new()
            .set_parent(&Parent(handle))
            .set_title(TITLE)
            .add_filter(FILTER, &EXTENSIONS)
            .pick_files()
    })?;
    Ok(move || pollster::block_on(selection).into_iter().flatten().map(|file| file.path().to_path_buf()).collect())
}
