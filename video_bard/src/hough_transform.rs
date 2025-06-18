fn laplacian(image1: &[u8], image2: &[u8]) -> Vec<bool> {
    // Convert the images into `&mut [Srgb<u8>]` and `&[Srgb<u8>]` without copying.
    let image1: &[Lumaa<Srgb, u8>] = image1.components_as();
    let image2: &[Lumaa<Srgb, u8>] = image2.components_as();
    let mut ret = vec![];

    for (color1, color2) in image1.iter().zip(image2) {
        // Convert the colors to linear floating point format and give them transparency values.
        let color1_inner = color1;
        let color2_inner = color2;

        // Alpha blend `color2_alpha` over `color1_alpha`.
        let luma = color1_inner.luma as i32 - color2_inner.luma as i32;
        let luma = (luma + 255) / 2;
        // let slop = 1;
        let r = if luma == 129 { true } else { false };
        // let r = (luma) as u8;

        // Convert the color part back to `Srgb<u8>` and overwrite the value in image1.
        ret.push(r);
    }
    ret
}

fn convolute<const T: usize>(
    image1: &[f32],
    width: usize,
    height: usize,
    x_kernel: &[f32; T],
    y_kernel: &[f32; T],
) -> Vec<f32> {
    assert!((T % 2) == 1);
    let mut ret = vec![];
    let mut intermediate = vec![];
    ret.resize(width * height, 0f32);
    intermediate.resize(width * height, 0f32);
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let offset = T as i32 / 2;
            let left_x = x - offset;
            let right_x = x + offset;
            let start_x = left_x.max(0);
            let end_x = right_x.min(width as i32 - 1);
            let image_index = x + y * width as i32;

            let mut v = image1[image_index as usize] * (right_x - end_x + start_x - left_x) as f32;
            for x in start_x..=end_x {
                let image_index = x + y * width as i32;
                let kernel_index = x - left_x;
                v += image1[image_index as usize] * x_kernel[kernel_index as usize];
            }
            intermediate[x as usize + (y as usize * width)] = v;
        }
    }
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let offset = T as i32 / 2;
            let left_y = y - offset;
            let right_y = y + offset;
            let start_y = left_y.max(0);
            let end_y = right_y.min(height as i32 - 1);
            let image_index = x + y * width as i32;

            let mut v =
                intermediate[image_index as usize] * (right_y - end_y + start_y - left_y) as f32;
            for y in start_y..=end_y {
                let image_index = x + y * width as i32;
                let kernel_index = y - left_y;
                v += intermediate[image_index as usize] * y_kernel[kernel_index as usize];
            }
            ret[x as usize + y as usize * width] = v;
        }
    }

    // for y in 0..height as i32 {
    //     for x in 0..width as i32 {
    //         let image_index = x + y*width as i32;
    //         let pixel_value = ret[image_index as usize];
    //         let pixel_value = (pixel_value - lowest) / (highest - lowest);
    //         ret[image_index as usize] = pixel_value;
    //     }
    // }
    ret
}

struct Edge {
    direction: Vector2<f32>,
    location: Vector2<usize>,
}

fn hough_vote(edges: &[Edge], width: usize, height: usize, radius: u32) -> Vec<u8> {
    let mut ret = vec![0; width * height];
    for e in edges {
        let offset = e.direction * radius as f32;
        let mut x = e.location.x as i32;
        let mut y = e.location.y as i32;
        x -= offset.x as i32;
        y -= offset.y as i32;
        let index = x + y * width as i32;
        for in_x in -1..=1 {
            for in_y in -1..=1 {
                write_vote(width, height, &mut ret, x + in_x, y + in_y);
            }
        }

        x += offset.x as i32 * 2;
        y += offset.y as i32 * 2;
        let fuzz: i32 = 1;
        for in_x in -fuzz..=fuzz {
            for in_y in -fuzz..=fuzz {
                for _ in 0..=(fuzz - in_x.abs().max(in_y.abs())) {
                    write_vote(width, height, &mut ret, x + in_x, y + in_y);
                }
            }
        }
    }
    let mut highest = 0;
    for v in &ret {
        if *v > highest {
            highest = *v;
        }
    }
    ret
}

fn write_vote(width: usize, height: usize, ret: &mut Vec<u8>, x: i32, y: i32) {
    let index = x + y * width as i32;
    if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
        ret[index as usize] += 1;
    }
}

#[derive(Debug)]
struct Circle {
    x: i32,
    y: i32,
    r: i32,
    votes: u8
}

fn main() {
    env_logger::init();

    // run().expect("Wtf");
    // let name = "test_images/full_whiteboard_s1.png";
    let name = "test_images/full_whiteboard_s2.png";
    let name = "test_images/full_whiteboard_s3.png";
    // let name = "test_images/complex.png";
    let name = "test_images/full_whiteboard_s4.png";
    // let name = "test_images/dot_small.png";
    let test_img = ImageReader::open(&name)
        .expect("Could not find image")
        .decode()
        .expect("Could not decode image data.");
    // let grey = test_img.grayscale();
    let blurred = test_img.clone().blur(2.0);
    // let blurred2 = grey.blur(1.5);
    let width = test_img.width();
    let height = test_img.height();
    let nr: Vec<f32> = test_img
        .clone()
        .into_rgba8()
        .pixels()
        .map(|c| c.0[0] as f32 / 255.0)
        .collect();
    let ng: Vec<f32> = test_img
        .clone()
        .into_rgba8()
        .pixels()
        .map(|c| c.0[1] as f32 / 255.0)
        .collect();
    let nb: Vec<f32> = test_img
        .into_rgba8()
        .pixels()
        .map(|c| c.0[2] as f32 / 255.0)
        .collect();
    // let mut grey_content = grey.into_bytes();
    // let mut content = blurred.into_bytes();
    // let mut content2 = blurred2.into_bytes();
    // let edges = laplacian(content.as_slice(), content2.as_slice());
    let rdy = convolute(
        nr.as_slice(),
        width as usize,
        height as usize,
        &[1f32, 2f32, 1f32],
        &[-1f32, 0f32, 1f32],
    );
    let rdx = convolute(
        nr.as_slice(),
        width as usize,
        height as usize,
        &[-1f32, 0f32, 1f32],
        &[1f32, 2f32, 1f32],
    );
    let gdy = convolute(
        ng.as_slice(),
        width as usize,
        height as usize,
        &[1f32, 2f32, 1f32],
        &[-1f32, 0f32, 1f32],
    );
    let gdx = convolute(
        ng.as_slice(),
        width as usize,
        height as usize,
        &[-1f32, 0f32, 1f32],
        &[1f32, 2f32, 1f32],
    );
    let bdy = convolute(
        nr.as_slice(),
        width as usize,
        height as usize,
        &[1f32, 2f32, 1f32],
        &[-1f32, 0f32, 1f32],
    );
    let bdx = convolute(
        nb.as_slice(),
        width as usize,
        height as usize,
        &[-1f32, 0f32, 1f32],
        &[1f32, 2f32, 1f32],
    );
    let radius = 10;
    // let votes = hough_vote(edges, width, height, radius);
    let mut out_img_buf = ImageBuffer::<image::Rgb<u8>, Vec<u8>>::new(width, height);
    for x in 0..width {
        for y in 0..height {
            out_img_buf[(x, y)] = image::Rgb([0, 0, 0]);
        }
    }
    let mut edges = vec![];
    let mut circles: Vec<Circle> = vec![];
    for x in 0..width {
        for y in 0..height {
            let rdx = rdx[(x + y * width) as usize];
            let rdy = rdy[(x + y * width) as usize];
            let gdx = gdx[(x + y * width) as usize];
            let gdy = gdy[(x + y * width) as usize];
            let bdx = bdx[(x + y * width) as usize];
            let bdy = bdy[(x + y * width) as usize];
            let dx = (rdx * rdx + gdx * gdx + bdx * bdx).sqrt();
            let dy = (rdy * rdy + gdy * gdy + bdy * bdy).sqrt();
            let mg = (dx * dx + dy * dy).sqrt();

            let threshold = 0.25f32;
            // let threshold = threshold * threshold;
            if mg > threshold {
                // out_img_buf[(x, y)] = image::Rgb([255, 255,255]);
                edges.push(Edge {
                    direction: Vector2::new(dx, dy).normalize(),
                    location: Vector2::new(x as usize, y as usize),
                });
            }
        }
    }
    let mut vote_maps: Vec<Vec<u8>> = vec![];
    let radii: Vec<u32> = (4..=14).step_by(1).collect();
    for radius in &radii {
        let vote_map = hough_vote(edges.as_slice(), width as usize, height as usize, *radius);
        vote_maps.push(vote_map);
    }

    for z in 0..radii.len() {
        let r = radii[z];
        let vote_thresh = 30 + (r as f32 * 1.6) as u8; //(27 + ((r * r) as u8 / 5));
        let acc_threshhold = 13 - (r as f32 * 0.2) as u32;
        let mut vote_avg = 0;
        let mut vote_high = 0;
        let mut vote_low = i32::MAX;
        let mut acc_avg = 0;
        let mut acc_high = 0;
        let mut acc_low = i32::MAX;
        let mut samples = 0;
        let mut acc_samples = 0;
        let mut passed = 0;
        for x in 0..width {
            for y in 0..height {
                let votes = vote_maps[z][(x + y * width) as usize];
                if votes > vote_thresh {
                    samples += 1;
                    let mut acc: u32 = 0;
                    let mut total: u32 = 0;
                    let mut highest = 0;
                    let limit = r as i32;
                    let mut is_local_highest = true;
                    for in_x in -limit..=limit {
                        for in_y in -limit..=limit {
                            for in_z in -5..=5 {
                                if in_x == 0 && in_y == 0 && in_z == 0 {
                                    continue;
                                }
                                let x = x as i32 + in_x;
                                let y = y as i32 + in_y;
                                let z = z as i32 + in_z;
                                if x < 0
                                    || x >= width as i32
                                    || y < 0
                                    || y >= height as i32
                                    || z < 0
                                    || z >= radii.len() as i32
                                {
                                    continue;
                                }
                                let v = vote_maps[z as usize][(x + y * width as i32) as usize];
                                acc += v as u32;
                                total += 1;
                                if v > votes {
                                    is_local_highest = false;
                                } else if (v == votes) {
                                    if in_x > 0 {
                                        is_local_highest = false;
                                        continue;
                                    }
                                    if in_x == 0 && in_y > 0 {
                                        is_local_highest = false;
                                        continue;
                                    }
                                    if in_x == 0 && in_y == 0 && in_z > 0 {
                                        is_local_highest = false;
                                        continue;
                                    }
                                }
                            }
                        }
                    }

                    acc = acc / total;

                    if (is_local_highest)
                    {
                        // info!("votes = {} acc = {}", votes, acc);
                        if (acc > acc_threshhold) {
                            // info!("IGNORED");
                            continue;
                        }
                        passed += 1;
                        circles.push(Circle {
                            x: x as i32,
                            y: y as i32,
                            r: r as i32,
                            votes
                        });
                        acc_avg += acc as i32;
                        if (acc as i32) < acc_low {
                            acc_low = acc as i32;
                        }
                        if (acc as i32) > acc_high {
                            acc_high = acc as i32;
                        }
                        acc_samples += 1;
                    }
                    vote_avg += votes as i32;
                    if (votes as i32) < vote_low {
                        vote_low = votes as i32;
                    }
                    if (votes as i32) > vote_high {
                        vote_high = votes as i32;
                    }
                }
                
            }
        }
        info!(
            "================== Radius {} =================",
            r
        );
        info!("{} votes h {}, l {}, a {} > {}", samples, vote_high, vote_low, if samples > 0 {vote_avg / samples} else {-1}, vote_thresh);
        info!("{} accs  h {}, l {}, a {} < {}", acc_samples, acc_high, acc_low, if acc_samples > 0 {acc_avg / acc_samples} else {-1}, acc_threshhold);
        info!("Passed {}", passed);
    }
    
    for x in 0..width {
        for y in 0..height {
            let r = (nr[(x + y * width) as usize] * 255f32) as u8;
            let g = (ng[(x + y * width) as usize] * 255f32) as u8;
            let b = (nb[(x + y * width) as usize] * 255f32) as u8;
            let c = out_img_buf[(x, y)];
            out_img_buf[(x, y)] = image::Rgb([r, g, b]);
        }
    }
    
    for circle in circles {
        let f = blurred.get_pixel(circle.x as u32, circle.y as u32);
        let fill = Srgb::<u8>::new(f.0[0],f.0[1],f.0[2]).into_linear::<f32>();
        let mut fill = Lch::from_color(fill.into_linear()).saturate(1.0);
        fill.set_hue((((fill.hue.into_degrees() / 30.0) as i32) as f32) * 30.0);
        let fill = LinSrgb::from_color(fill);

        
        for x in -circle.r..circle.r {
            for y in -circle.r..circle.r {
                if x == 0 && y == 0 {
                    let fill:Srgb<u8> = fill.into_encoding();
                    out_img_buf[(x as u32, y as u32)] = image::Rgb([fill.red,fill.green,fill.blue]);
                    continue;
                }
                let pos = Vector2::new(x as f32,y as f32);
                let dist = (pos).magnitude_squared();
                let r2 = (circle.r * circle.r) as f32;
                let x = x + circle.x;
                let y = y + circle.y;
                if dist < r2 {
                    let c = out_img_buf[(x as u32, y as u32)];
                    let original = Srgb::<u8>::new(c.0[0],c.0[1],c.0[2]).into_linear::<f32>();
                    let mix: Srgb<u8> = original.mix(fill, 1.0).into_encoding();
                    out_img_buf[(x as u32, y as u32)] = image::Rgb([mix.red,mix.green,mix.blue]);
                }                
            }
        }
    }
    let out_img = DynamicImage::from(out_img_buf);
    out_img.save("out.bmp").expect("Could not write output");
}