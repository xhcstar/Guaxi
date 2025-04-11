// use anyhow::Result;
// use image::DynamicImage;
// use tesseract::Tesseract;

// pub fn ocr_image(image: &DynamicImage, language: &str) -> Result<String> {
//     let mut tesseract = Tesseract::new(None, Some(language))?;
    
//     let rgb_image = image.to_rgb8();
//     let (width, height) = rgb_image.dimensions();

//     tesseract.set_image_from_u8(
//         &rgb_image.into_raw(),
//         width as i32,
//         height as i32,
//         3,
//         width as i32 * 3,
//     )?;

//     Ok(tesseract.get_text()?.trim().to_string())
// }