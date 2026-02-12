use image::imageops::overlay;
use image::{ImageBuffer, ImageFormat, Rgba};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

#[pyclass]
pub struct CaptchaGenerator {
    emoji_map: HashMap<char, ImageBuffer<Rgba<u8>, Vec<u8>>>,
    emojis: Vec<char>,
}

#[pyclass]
pub struct CaptchaData {
    #[pyo3(get)]
    pub correct_emoji: char,
    #[pyo3(get)]
    pub image_emojis: Vec<char>,
    #[pyo3(get)]
    pub keyboard_emojis: Vec<char>,
    #[pyo3(get)]
    pub image: Py<PyBytes>,
}

#[pymethods]
impl CaptchaGenerator {
    #[new]
    #[pyo3(signature = (emojis_path, format = None))]
    pub fn new(emojis_path: PathBuf, format: Option<&str>) -> Self {
        let mut emoji_map: HashMap<char, ImageBuffer<Rgba<u8>, Vec<u8>>> = HashMap::new();
        let mut emojis: Vec<char> = Vec::new();
        let format = Some(OsStr::new(format.unwrap_or("png")));

        if let Ok(entries) = fs::read_dir(&emojis_path) {
            entries
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    if path.extension() == format {
                        image::open(&path).ok().map(|img| (path, img))
                    } else {
                        None
                    }
                })
                .filter_map(|(path, image)| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .and_then(|stem| u32::from_str_radix(stem, 16).ok())
                        .and_then(char::from_u32)
                        .map(|emoji_char| (emoji_char, image))
                })
                .for_each(|(emoji_char, image)| {
                    emoji_map.insert(emoji_char,  image.into_rgba8().clone());
                    emojis.push(emoji_char);
                });
        }

        CaptchaGenerator { emoji_map, emojis }
    }

    pub fn generate(
        &self,
        image_emojis_count: usize,
        keyboard_emojis_count: usize,
        py: Python,
    ) -> PyResult<CaptchaData> {
        let mut rng = thread_rng();
        let num_emojis = image_emojis_count.min(self.emoji_map.len());

        let selected_emojis: Vec<_> = self
            .emojis
            .choose_multiple(&mut rng, num_emojis + keyboard_emojis_count - 1)
            .cloned()
            .collect();

        let mut image_emojis = selected_emojis[0..num_emojis].to_vec();
        let mut keyboard_emojis = selected_emojis[num_emojis - 1..].to_vec();
        let correct_emoji = image_emojis[num_emojis - 1];

        image_emojis.shuffle(&mut rng);
        keyboard_emojis.shuffle(&mut rng);

        let selected_images: Vec<_> = image_emojis
            .iter()
            .map(|emoji| self.emoji_map.get(emoji).unwrap())
            .collect();

        let output_width = 550;
        let output_height = 180;
        let spacing = 20;

        let mut output_image: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(output_width, output_height, Rgba([255, 255, 255, 255]));

        let total_image_width: i32 = selected_images.iter().map(|img| img.width() as i32).sum();
        let total_spacing_width = spacing * (num_emojis - 1);
        let total_width_needed = total_image_width + total_spacing_width as i32;

        let mut x_offset: i32 = (output_width as i32 - total_width_needed) / 2;
        for img in selected_images {
            let y_offset = (output_height - img.height()) / 2;
            overlay(&mut output_image, img, x_offset as i64, y_offset as i64);
            x_offset += img.width() as i32 + spacing as i32;
        }

        let mut buf = Vec::new();
        output_image
            .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();

        Ok(CaptchaData {
            correct_emoji,
            image: PyBytes::new(py, &buf).into(),
            image_emojis,
            keyboard_emojis,
        })
    }
}
