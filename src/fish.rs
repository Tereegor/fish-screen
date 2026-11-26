use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{
    AnimationDecoder,
    codecs::gif::GifDecoder,
    imageops::{FilterType, resize},
};
use ratatui::layout::Rect;
use std::{
    io::{Cursor, Write},
    time::Duration,
};

pub struct Fish {
    pub delay: Duration,
    pixels: image::RgbaImage,
    data: String,
}

pub fn load(bytes: &[u8]) -> Result<Vec<Fish>> {
    let frames = GifDecoder::new(Cursor::new(bytes))?.into_frames();
    let mut fish = Vec::new();
    for frame in frames {
        let frame = frame?;
        let (ms, div) = frame.delay().numer_denom_ms();
        let mut pixels = frame.into_buffer();
        for pixel in pixels.pixels_mut() {
            let alpha = u32::from(pixel[3]);
            for value in &mut pixel.0[..3] {
                *value = ((u32::from(*value) * alpha + 255 * (255 - alpha)) / 255) as u8;
            }
            pixel[3] = 255;
        }
        fish.push(Fish {
            delay: Duration::from_millis(u64::from((ms / div).max(10))),
            pixels,
            data: String::new(),
        });
    }
    ensure!(!fish.is_empty(), "GIF contains no frames");
    Ok(fish)
}

impl Fish {
    pub fn fit(&self, box_: Rect, cell: (f64, f64), step: u16) -> Rect {
        let cols = box_.width / step;
        let rows = box_.height / step;
        let aspect =
            f64::from(self.pixels.width()) * cell.1 / (f64::from(self.pixels.height()) * cell.0);
        let width = (f64::from(rows) * aspect)
            .round()
            .clamp(1., f64::from(cols)) as u16
            * step;
        let height = (f64::from(cols) / aspect)
            .round()
            .clamp(1., f64::from(rows)) as u16
            * step;
        Rect::new(
            box_.x + (box_.width - width) / 2,
            box_.y + (box_.height - height) / 2,
            width,
            height,
        )
    }

    pub fn shades(&self, width: u16, height: u16) -> Vec<u8> {
        resize(
            &self.pixels,
            width.into(),
            height.into(),
            FilterType::Triangle,
        )
        .pixels()
        .map(|pixel| {
            let [red, green, blue, _] = pixel.0;
            let gray = (30 * red as usize + 59 * green as usize + 11 * blue as usize) / 100;
            (255 - gray) as u8
        })
        .collect()
    }

    pub fn show(
        &mut self,
        out: &mut impl Write,
        area: Rect,
        fg: [u8; 3],
        bg: [u8; 3],
    ) -> Result<()> {
        if self.data.is_empty() {
            let mut pixels = self.pixels.clone();
            for pixel in pixels.pixels_mut() {
                let [red, green, blue, _] = pixel.0;
                let gray = ((30 * u32::from(red) + 59 * u32::from(green) + 11 * u32::from(blue))
                    / 100)
                    .min(254);
                for channel in 0..3 {
                    pixel[channel] = ((u32::from(fg[channel]) * (254 - gray)
                        + u32::from(bg[channel]) * gray)
                        / 254) as u8;
                }
            }
            self.data = STANDARD.encode(pixels.as_raw());
        }
        write!(out, "\x1b7\x1b[{};{}H", area.y + 1, area.x + 1)?;
        let mut chunks = self.data.as_bytes().chunks(4096).peekable();
        let mut header = format!(
            "a=T,f=32,s={},v={},i=7291,c={},r={},q=2,C=1,",
            self.pixels.width(),
            self.pixels.height(),
            area.width,
            area.height
        );
        while let Some(chunk) = chunks.next() {
            write!(
                out,
                "\x1b_G{header}m={};",
                u8::from(chunks.peek().is_some())
            )?;
            out.write_all(chunk)?;
            out.write_all(b"\x1b\\")?;
            header.clear();
        }
        out.write_all(b"\x1b8")?;
        out.flush()?;
        Ok(())
    }
}
