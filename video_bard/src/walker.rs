use log::info;
use nalgebra::Rotation2;
use nalgebra::Vector2;
use nalgebra::Vector3;

use super::MAGENTA;

use super::RED;

use super::convert_rgba;

use super::DotDescription;

use image::DynamicImage;
use image::GenericImage;
use image::GenericImageView;

trait ProtectedPut {
    fn put_protected(&mut self, x: i32, y: i32, pixel: image::Rgba<u8>);
    fn get_protected(&self, x: i32, y: i32) -> image::Rgba<u8>;
}

const WHITE: image::Rgba<u8> = image::Rgba([255u8, 255u8, 255u8, 255u8]);

impl ProtectedPut for DynamicImage {
    fn put_protected(&mut self, x: i32, y: i32, pixel: image::Rgba<u8>) {
        if x < 0 || y < 0 || x >= self.width() as i32 || y >= self.height() as i32 {
            return;
        }
        self.put_pixel(x as u32, y as u32, pixel);
    }
    fn get_protected(&self, x: i32, y: i32) -> image::Rgba<u8> {
        if x < 0 || y < 0 || x >= self.width() as i32 || y >= self.height() as i32 {
            WHITE
        } else {
            self.get_pixel(x as u32, y as u32)
        }
    }
}

pub(crate) const base_vectors: [Vector2<f32>;8] = [
    Vector2::new(1f32, 0f32),
    Vector2::new(0.75, 0.75),
    Vector2::new(0f32, 1f32),
    Vector2::new(-0.75, 0.75),
    Vector2::new(-1f32, 0f32),
    Vector2::new(-0.75, -0.75),
    Vector2::new(0f32, -1f32),
    Vector2::new(0.75, -0.75),
];

pub(crate) fn match_dots(test_img: DynamicImage, out_img: &mut DynamicImage) -> Vec<DotDescription> {
    let max_size = 25;
    let min_size = 4;
    let mut dots: Vec<DotDescription> = vec![];
    let mut test_tally = 0;
    let mut location_tally = 0;
    let grid_gap = 5;
    for mut x in (0..test_img.width() as i32).step_by(grid_gap) {
        for mut y in (0..test_img.height() as i32).step_by(grid_gap) {
            // let x = 65f32;
            // let y = 65f32;
            let start = Vector2::new(x as f32, y as f32);
            let mut center = Vector2::new(x as f32, y as f32);
            location_tally += 1;
            // let mut x = 58;
            // let mut y = 63;
            let mut reference_colour = convert_rgba(&test_img.get_protected(center.x as i32, center.y as i32));
            let mut is_dot = false;
            let mut dot_size = 0;
            let mut successful_tests = 0;
            let mut total_tests = 1;
            let mut inner_distance = 1;
            let mut outer_distance = max_size;
            let mut growing = true;
            while inner_distance < outer_distance {
                if growing {
                    let (match_center, matches, average_colour) = match_ring(
                        &test_img,
                        &center,
                        &reference_colour,
                        inner_distance,
                        out_img,
                    );
                    successful_tests += matches;
                    total_tests += base_vectors.len();
                    if matches < 4 {
                        is_dot = true;
                        dot_size = inner_distance;
                        break;
                    } else {
                        if inner_distance > min_size {
                            growing = false
                        }
                    }
                    center = match_center;
                    reference_colour = reference_colour.lerp(&average_colour, successful_tests as f32 / total_tests as f32);
                    inner_distance += 2
                } else {
                    let (match_center, matches, average_colour) = match_ring(
                        &test_img,
                        &center,
                        &reference_colour,
                        outer_distance,
                        out_img,
                    );
                    if outer_distance == max_size && matches > 5 {
                        break;
                    }
                    if matches < 2 {
                        growing = true;
                    }
                    outer_distance -= 2;
                }
                test_tally += base_vectors.len();
            }
            if inner_distance > outer_distance {
                is_dot = true;
                dot_size = inner_distance as i32;
            }
            if dot_size < min_size {
                is_dot = false;
            }
            if is_dot {
                let dot_walk_distance = (start - center).magnitude_squared();
                let mut c = RED;
                c.0[0] = ((dot_walk_distance / max_size as f32).min(1f32) * 255f32) as u8;
                // out_img.put_protected(center.x as i32, center.y as i32, c);
                out_img.put_protected(center.x as i32, center.y as i32, MAGENTA);
                // for v in base_vectors {
                //     out_img.put_protected((center.x + v.x * dot_size as f32) as i32, (center.y + v.y * dot_size as f32) as i32, MAGENTA);
                // }
                dots.push(DotDescription {
                    pos: center,
                    size: dot_size as u32,
                });
            }
        }
    }
    info!(
        "Total checks done {} over {} locations",
        test_tally, location_tally
    );
    dots
}

pub(crate) fn match_ring(
    test_img: &DynamicImage,
    center: &Vector2<f32>,
    reference_colour: &Vector3<f32>,
    distance: i32,
    out_img: &mut DynamicImage,
) -> (
    Vector2<f32>,
    i32,
    Vector3<f32>,
) {
    let tolerance = 30f32;
    let tolerance = tolerance * tolerance;
    let mut average_colour = *reference_colour;
    let rot = Rotation2::new(distance as f32);
    let mut layer_center = *center;
    let mut matches = 0;
    for offset in base_vectors {
        let offset = rot.transform_vector(&offset);

        let check_pos = center + offset * distance as f32;

        let new_colour = convert_rgba(&test_img.get_protected(check_pos.x as i32, check_pos.y as i32));
        let mag_sqrd = (reference_colour - new_colour).magnitude_squared();
        if mag_sqrd < tolerance {
            // out_img.put_protected(check_pos.x as i32, check_pos.y as i32, GREEN);
            matches += 1;
            average_colour += new_colour;
            layer_center += check_pos;
        } else {
            // out_img.put_protected(check_pos.x as i32, check_pos.y as i32, RED);
        }
    }
    average_colour /= 1f32 + matches as f32;
    layer_center /= 1f32 + matches as f32;
    // out_img.put_protected(layer_center.x as i32, layer_center.y as i32, BLUE);

    (layer_center, matches, average_colour)
}