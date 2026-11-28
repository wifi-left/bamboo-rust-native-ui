// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 wifi-left

//! Application-owned images for widgets.
//!
//! The backend already draws RGBA with a per-`ImageId` cache, but a widget has
//! no way to reach the pixels: the application owns them and the widget only
//! knows an id. A page therefore registers owned pixels with
//! [`crate::Ui::set_image`] and widgets ask for them through
//! [`crate::WidgetPaint::image`]. The store holds one `Arc` per image, so a
//! cover shared by a grid, an avatar and a now-playing card costs one copy.
//!
//! Pixels are RGBA in row order, exactly `width * height * 4` bytes.

use std::collections::HashMap;
use std::sync::Arc;

use crate::backend::ImageId;
use crate::color::Color;

/// Owned RGBA pixels registered with a page.
#[derive(Clone)]
pub struct UiImage {
    pub id: ImageId,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl UiImage {
    /// Build an image, validating the buffer length. A cover decoded by the
    /// application is usually already an `Arc<[u8]>`; anything byte-sliceable
    /// is copied once here.
    pub fn new(
        id: ImageId,
        width: u32,
        height: u32,
        rgba: impl Into<Arc<[u8]>>,
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let rgba = rgba.into();
        if rgba.len() != width as usize * height as usize * 4 {
            return None;
        }
        Some(Self {
            id,
            width,
            height,
            rgba,
        })
    }

    /// A one-color image. Useful for placeholders, tests and demos without an
    /// asset pipeline.
    pub fn solid(id: ImageId, width: u32, height: u32, color: Color) -> Self {
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..width as usize * height as usize {
            rgba.extend_from_slice(&[color.r, color.g, color.b, 255]);
        }
        Self {
            id,
            width,
            height,
            rgba: rgba.into(),
        }
    }

    /// A vertical two-color gradient. The cover-wall demo uses it instead of
    /// shipping artwork.
    pub fn gradient(id: ImageId, width: u32, height: u32, top: Color, bottom: Color) -> Self {
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            let t = if height > 1 {
                y as f32 / (height - 1) as f32
            } else {
                0.0
            };
            let color = top.mix(bottom, t);
            for _ in 0..width {
                rgba.extend_from_slice(&[color.r, color.g, color.b, 255]);
            }
        }
        Self {
            id,
            width,
            height,
            rgba: rgba.into(),
        }
    }
}

/// Images registered on one page, keyed by [`ImageId`].
#[derive(Default, Clone)]
pub struct ImageStore {
    images: HashMap<ImageId, UiImage>,
}

impl ImageStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace an image. Replacing keeps the id stable, so widgets
    /// that already reference it repaint the new pixels.
    pub fn insert(&mut self, image: UiImage) {
        self.images.insert(image.id, image);
    }

    pub fn remove(&mut self, id: ImageId) -> Option<UiImage> {
        self.images.remove(&id)
    }

    pub fn get(&self, id: ImageId) -> Option<&UiImage> {
        self.images.get(&id)
    }

    pub fn contains(&self, id: ImageId) -> bool {
        self.images.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    pub fn clear(&mut self) {
        self.images.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_a_mismatched_buffer() {
        let id = ImageId(1);
        assert!(UiImage::new(id, 2, 2, vec![0u8; 15]).is_none());
        assert!(UiImage::new(id, 0, 2, vec![0u8; 0]).is_none());
        assert!(UiImage::new(id, 2, 2, vec![0u8; 16]).is_some());
    }

    #[test]
    fn solid_and_gradient_have_the_expected_size() {
        let solid = UiImage::solid(ImageId(2), 3, 5, Color::new(1, 2, 3));
        assert_eq!(solid.rgba.len(), 3 * 5 * 4);
        assert_eq!(&solid.rgba[..4], &[1, 2, 3, 255]);

        let gradient = UiImage::gradient(
            ImageId(3),
            2,
            3,
            Color::new(0, 0, 0),
            Color::new(255, 255, 255),
        );
        assert_eq!(gradient.rgba.len(), 2 * 3 * 4);
        // First row is the top color, last row the bottom color.
        assert_eq!(&gradient.rgba[..3], &[0, 0, 0]);
        let last = gradient.rgba.len() - 4;
        assert_eq!(&gradient.rgba[last..last + 3], &[255, 255, 255]);
    }

    #[test]
    fn store_replaces_by_id_and_removes() {
        let id = ImageId(4);
        let mut store = ImageStore::new();
        store.insert(UiImage::solid(id, 1, 1, Color::new(1, 1, 1)));
        store.insert(UiImage::solid(id, 2, 2, Color::new(2, 2, 2)));
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(id).unwrap().width, 2);
        assert!(store.remove(id).is_some());
        assert!(store.is_empty());
    }
}
