use super::{capture_full_screen, Toolbar};
use eframe::egui;
use egui::epaint::RectShape;
use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use tracing::{info, error, warn};

pub struct ScreenshotApp {
    pub toolbar: Toolbar,
    pub config: super::capture::CaptureConfig,
    pub image: Option<Vec<u8>>,
    pub selection: Option<(egui::Pos2, egui::Pos2)>,
    pub drawing: VecDeque<egui::Pos2>,
    pub is_active: bool,
}

impl ScreenshotApp {
    pub fn new() -> Self {
        info!("Creating new ScreenshotApp instance");
        Self {
            toolbar: Toolbar::default(),
            config: super::capture::CaptureConfig::default(),
            image: None,
            selection: None,
            drawing: VecDeque::new(),
            is_active: true,
        }
    }

    pub fn show_standalone() -> eframe::Result<()> {
        info!("Starting standalone screenshot tool");
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([800.0, 600.0])
                .with_always_on_top(),
            ..Default::default()
        };

        info!("Running native eframe application");
        eframe::run_native(
            "Screenshot Tool",
            options,
            Box::new(|_cc| {
                info!("Creating app for standalone mode");
                let app = Self::new();
                info!("App created successfully");
                Ok(Box::new(app))
            }),
        )
    }

    pub fn show_with_image(image_data: &[u8]) -> eframe::Result<()> {
        info!("Starting screenshot tool with image data");
        info!("Image data length: {} bytes", image_data.len());
        
        // Validate image data first
        if let Err(e) = image::load_from_memory(image_data) {
            error!("Invalid image data: {}", e);
            std::process::exit(1);
        }
    
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_fullscreen(true)
                .with_always_on_top()
                .with_active(true),
            ..Default::default()
        };
    
        eframe::run_native(
            "Screenshot Tool",
            options,
            Box::new(|_cc| {
                let mut app = Self::new();
                app.image = Some(image_data.to_vec());
                Ok(Box::new(app))
            }),
        )
    }
}

impl eframe::App for ScreenshotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        info!("App update cycle started");
        
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        info!("Cursor set to crosshair");

        egui::CentralPanel::default().show(ctx, |ui| {
            info!("Rendering central panel");
            
            self.toolbar.ui(ui);
            info!("Toolbar rendered");

            if let Some(image_data) = &self.image {
                info!("Attempting to load image from memory");
                match image::load_from_memory(image_data) {
                    Ok(image) => {
                        let size = [image.width() as usize, image.height() as usize];
                        info!("Image loaded successfully, size: {:?}", size);
                        
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            size, 
                            image.to_rgba8().as_raw()
                        );
                        let texture = ctx.load_texture("screenshot", color_image, Default::default());
                        ui.image(&texture);
                        info!("Image texture created and displayed");
                    },
                    Err(e) => {
                        error!("Failed to load image: {}", e);
                    }
                }
            } else {
                warn!("No image data available to display");
            }

            let response = ui.interact(
                ui.max_rect(),
                egui::Id::new("screenshot_area"),
                egui::Sense::click_and_drag(),
            );
            info!("Interaction area setup complete");

            if response.drag_started() {
                if let Some(pos) = ctx.pointer_latest_pos() {
                    self.selection = Some((pos, pos));
                    info!("Selection started at position: {:?}", pos);
                }
            }

            if let Some(pos) = ctx.pointer_latest_pos() {
                if let Some((start, _)) = &mut self.selection {
                    *start = pos;
                    info!("Selection updated to position: {:?}", pos);
                }
            }

            if let Some((start, end)) = self.selection {
                let rect = egui::Rect::from_two_pos(start, end);
                ui.painter().add(egui::Shape::Rect(RectShape {
                    rect,
                    corner_radius: egui::CornerRadius::ZERO,
                    fill: egui::Color32::TRANSPARENT,
                    stroke: egui::Stroke::new(2.0, egui::Color32::RED),
                    stroke_kind: egui::StrokeKind::Middle,
                    round_to_pixels: None,
                    blur_width: 0.0,
                    brush: None,
                }));
                info!("Selection rectangle drawn from {:?} to {:?}", start, end);
            }
        });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            info!("ESC key pressed, exiting application");
            std::process::exit(0);
        }
        
        info!("App update cycle completed");
    }
}