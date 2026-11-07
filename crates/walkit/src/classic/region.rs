// SPDX-License-Identifier: GPL-3.0-or-later
//! `region.txt`: a polygon per window look (`[Normal]`, `[WindowShade]`, `[Equalizer]`,
//! `[EqualizerWS]`), as Winamp makes a window region of it: the pixels whose centers lie inside
//! the polygon, as GDI fills a polygon region, here as a row of boxes per scanline.

use std::collections::HashMap;

use crate::draw::{Rect, Region};

/// The polygons of `text`, by section name in lowercase; each section's `PointList` as points,
/// `NumPoints` splitting them into polygons.
pub fn parse(text: &str) -> HashMap<String, Vec<Vec<(i32, i32)>>> {
    let mut out = HashMap::new();
    let mut section: Option<String> = None;
    let mut counts: Vec<usize> = Vec::new();
    let mut points: Vec<(i32, i32)> = Vec::new();
    let flush = |out: &mut HashMap<String, Vec<Vec<(i32, i32)>>>,
                 section: &Option<String>,
                 counts: &[usize],
                 points: &[(i32, i32)]| {
        let Some(name) = section else {
            return;
        };
        let mut polygons = Vec::new();
        let mut at = 0;
        let counts: Vec<usize> = if counts.is_empty() {
            vec![points.len()]
        } else {
            counts.to_vec()
        };
        for n in counts {
            let end = (at + n).min(points.len());
            if end > at {
                polygons.push(points[at..end].to_vec());
            }
            at = end;
        }
        out.insert(name.clone(), polygons);
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            flush(&mut out, &section, &counts, &points);
            section = Some(name.trim().to_lowercase());
            counts.clear();
            points.clear();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let numbers: Vec<i32> = value
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        match key.trim().to_lowercase().as_str() {
            "numpoints" => counts = numbers.iter().map(|n| (*n).max(0) as usize).collect(),
            "pointlist" => points.extend(numbers.chunks_exact(2).map(|p| (p[0], p[1]))),
            _ => {}
        }
    }
    flush(&mut out, &section, &counts, &points);
    out
}

/// The region of `polygons` inside a `w` x `h` window: a box per run of pixels whose centers
/// the polygons cover (winding rule, as GDI fills a polygon region).
pub fn region(polygons: &[Vec<(i32, i32)>], w: i32, h: i32) -> Region {
    let mut boxes = Vec::new();
    for y in 0..h {
        let cy = f64::from(y) + 0.5;
        // the crossings of this row's center line, each with the edge's direction
        let mut crossings: Vec<(f64, i32)> = Vec::new();
        for poly in polygons {
            let n = poly.len();
            for i in 0..n {
                let (x1, y1) = poly[i];
                let (x2, y2) = poly[(i + 1) % n];
                if y1 == y2 {
                    continue;
                }
                let (fy1, fy2) = (f64::from(y1), f64::from(y2));
                let (lo, hi) = if fy1 < fy2 { (fy1, fy2) } else { (fy2, fy1) };
                if cy < lo || cy >= hi {
                    continue;
                }
                let t = (cy - fy1) / (fy2 - fy1);
                let x = f64::from(x1) + t * f64::from(x2 - x1);
                crossings.push((x, if y2 > y1 { 1 } else { -1 }));
            }
        }
        crossings.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut winding = 0;
        let mut start: Option<f64> = None;
        for (x, dir) in crossings {
            let before = winding;
            winding += dir;
            if before == 0 && winding != 0 {
                start = Some(x);
            } else if before != 0 && winding == 0 {
                if let Some(s) = start.take() {
                    // pixels whose centers lie in [s, x)
                    let left = (s - 0.5).ceil().max(0.0) as i32;
                    let right = ((x - 0.5).ceil() as i32).min(w);
                    if right > left {
                        boxes.push(Region::Box(Rect::new(left, y, right - left, 1)));
                    }
                }
            }
        }
    }
    Region::Op {
        base: None,
        add: boxes,
        keep: Vec::new(),
        cut: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, region};
    use crate::draw::{Rect, Region};

    #[test]
    fn a_polygon_along_pixel_corners_covers_the_pixels_inside_it() {
        let text = "[Normal]\nNumPoints=8\nPointList=0,2, 2,2, 2,0, 6,0, 6,2, 8,2, 8,4, 0,4\n";
        let polys = parse(text);
        let r = region(&polys["normal"], 8, 4);
        let Region::Op { add, .. } = r else {
            panic!("boxes")
        };
        assert_eq!(
            add,
            vec![
                Region::Box(Rect::new(2, 0, 4, 1)),
                Region::Box(Rect::new(2, 1, 4, 1)),
                Region::Box(Rect::new(0, 2, 8, 1)),
                Region::Box(Rect::new(0, 3, 8, 1)),
            ]
        );
    }

    #[test]
    fn numpoints_splits_the_list_into_polygons_and_sections_are_lowercase() {
        let text = "[WindowShade]\nNumPoints=4,4\nPointList=0,0,1,0,1,1,0,1, 3,0,4,0,4,1,3,1\n";
        let polys = parse(text);
        assert_eq!(polys["windowshade"].len(), 2);
        let Region::Op { add, .. } = region(&polys["windowshade"], 5, 1) else {
            panic!("boxes")
        };
        assert_eq!(add.len(), 2);
    }
}
