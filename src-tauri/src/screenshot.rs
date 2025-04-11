pub mod capture;
pub mod ocr;
pub mod tools;
pub mod ui;

pub use capture::{CaptureConfig, capture_full_screen};
// pub use ocr::ocr_image;
pub use tools::Toolbar;
pub use ui::ScreenshotApp;