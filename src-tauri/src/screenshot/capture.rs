use anyhow::{Context, Result};
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use screenshots::Screen;
use std::io::Cursor;
use tokio::task;

#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub include_cursor: bool,
    pub ocr_language: Option<String>,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            include_cursor: false,
            ocr_language: Some("eng+chi_sim".into()),
        }
    }
}

pub async fn capture_full_screen(_config: &CaptureConfig) -> Result<Vec<u8>> {
    let screens = Screen::all().context("Failed to get screens")?;
    let screen = screens.first().cloned().context("No screens found")?;

    let screenshot = task::spawn_blocking(move || screen.capture())
        .await?
        .context("Failed to capture screen")?;

    let width = screenshot.width();
    let height = screenshot.height();
    let pixels = screenshot.into_raw();

    let mut bytes = Vec::new();
    let encoder = PngEncoder::new(&mut bytes);
    encoder
        .write_image(&pixels, width, height, ColorType::Rgba8.into())
        .context("Failed to encode PNG")?;

    Ok(bytes)
}
