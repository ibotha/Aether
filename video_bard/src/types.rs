use std::ops::Deref;

use nalgebra::{Vector2, Vector3};

pub trait Color {
    
}

pub struct Image<T> where T: Color + Copy {
    data: Vec<T>,
    width: usize,
    height: usize,
}

impl<T> Image<T> where T:Color + Copy {
    pub fn new(width: usize, height: usize) -> Self {
        let mut data = Vec::new();
        data.reserve_exact(width * height);
        Self {
            data,
            width,
            height
        }
    }

    pub fn get_pixel(&self, pos: Vector2<usize>) -> T {
        return self.data[pos.x + pos.y * self.width];
    }
}

#[derive(Copy, Clone)]
pub struct RGB(Vector3<f32>);

impl Color for RGB {}

impl Deref for RGB {
    type Target = Vector3<f32>;

    fn deref(&self) -> &Self::Target {
        return &self.0;
    }
}

#[derive(Copy, Clone)]
pub struct HSV(Vector3<f32>);

impl Color for HSV {}

impl Into<RGB> for HSV {
    fn into(self) -> RGB {
        todo!()
    }
}

impl Deref for HSV {
    type Target = Vector3<f32>;

    fn deref(&self) -> &Self::Target {
        return &self.0;
    }
}

impl Into<HSV> for RGB {
    fn into(self) -> HSV {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let rgb = Image::<RGB>::new(1920, 1080);
    }
}