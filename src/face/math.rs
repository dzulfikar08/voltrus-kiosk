//! Pure face math, a line-for-line port of console/src/lib/face/faceMath.ts
//! so the kiosk and the browser produce the same embeddings: the YuNet decode
//! (OpenCV FaceDetectorYN::postProcess), NMS, the 5-point alignment onto the
//! SFace template, the MiniFASNet crop and softmax, and image resampling.

/// Stored with every embedding; the server matches only this model.
pub const FACE_MODEL: &str = "sface-2021dec-int8";
pub const EMBEDDING_DIM: usize = 128;

pub const DET_MAX: u32 = 640;
pub const DET_DIVISOR: u32 = 32;
pub const DET_THRESH: f32 = 0.6;
pub const NMS_IOU: f32 = 0.3;
pub const STRIDES: [u32; 3] = [8, 16, 32];
pub const REC_SIZE: u32 = 112;

pub const ANTISPOOF_SIZE: u32 = 80;
pub const ANTISPOOF_MARGIN: f32 = 2.7;
pub const ANTISPOOF_THRESH: f32 = 0.5;

pub type BBox = [f32; 4];
pub type Point = [f32; 2];

#[derive(Debug, Clone)]
pub struct RawFace {
    pub bbox: BBox,
    pub score: f32,
    /// Eyes, nose, mouth corners in frame pixels.
    pub kps: [Point; 5],
}

pub fn iou(a: &BBox, b: &BBox) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = a[2].min(b[2]);
    let y2 = a[3].min(b[3]);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let area_a = (a[2] - a[0]).max(0.0) * (a[3] - a[1]).max(0.0);
    let area_b = (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
    inter / (area_a + area_b - inter).max(1e-9)
}

pub fn nms(mut cands: Vec<RawFace>, iou_thresh: f32) -> Vec<RawFace> {
    cands.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut keep: Vec<RawFace> = Vec::new();
    for c in cands {
        if keep.iter().all(|k| iou(&k.bbox, &c.bbox) < iou_thresh) {
            keep.push(c);
        }
    }
    keep
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub scale: f32,
    pub sw: u32,
    pub sh: u32,
    pub pw: u32,
    pub ph: u32,
}

/// Detector input geometry: downscale (never up) to a long side of 640,
/// pad to a multiple of 32.
pub fn detector_geometry(w: u32, h: u32) -> Geometry {
    let scale = (DET_MAX as f32 / w.max(h) as f32).min(1.0);
    let sw = ((w as f32 * scale).round() as u32).max(1);
    let sh = ((h as f32 * scale).round() as u32).max(1);
    Geometry {
        scale,
        sw,
        sh,
        pw: sw.div_ceil(DET_DIVISOR) * DET_DIVISOR,
        ph: sh.div_ceil(DET_DIVISOR) * DET_DIVISOR,
    }
}

pub struct YunetOutputs<'a> {
    pub cls: &'a [f32],
    pub obj: &'a [f32],
    pub bbox: &'a [f32],
    pub kps: &'a [f32],
}

/// YuNet outputs per stride (cls/obj [N], bbox [N,4], kps [N,10]) → frame
/// boxes and landmarks after NMS.
pub fn decode_yunet<'a>(
    get: impl Fn(u32) -> YunetOutputs<'a>,
    pw: u32,
    ph: u32,
    scale: f32,
    thresh: f32,
) -> Vec<RawFace> {
    let mut cands = Vec::new();
    for stride in STRIDES {
        let cols = (pw / stride) as usize;
        let rows = (ph / stride) as usize;
        let o = get(stride);
        let s = stride as f32;
        for r in 0..rows {
            for c in 0..cols {
                let idx = r * cols + c;
                if idx >= o.cls.len() || idx >= o.obj.len() {
                    continue;
                }
                let score = (o.cls[idx].clamp(0.0, 1.0) * o.obj[idx].clamp(0.0, 1.0)).sqrt();
                if score < thresh {
                    continue;
                }
                let cx = (c as f32 + o.bbox[idx * 4]) * s;
                let cy = (r as f32 + o.bbox[idx * 4 + 1]) * s;
                let w = o.bbox[idx * 4 + 2].exp() * s;
                let h = o.bbox[idx * 4 + 3].exp() * s;
                let mut kps = [[0.0; 2]; 5];
                for (n, p) in kps.iter_mut().enumerate() {
                    *p = [
                        (o.kps[idx * 10 + 2 * n] + c as f32) * s / scale,
                        (o.kps[idx * 10 + 2 * n + 1] + r as f32) * s / scale,
                    ];
                }
                cands.push(RawFace {
                    bbox: [
                        (cx - w / 2.0) / scale,
                        (cy - h / 2.0) / scale,
                        (cx + w / 2.0) / scale,
                        (cy + h / 2.0) / scale,
                    ],
                    score,
                    kps,
                });
            }
        }
    }
    nms(cands, NMS_IOU)
}

/// First EMBEDDING_DIM values, L2-normalised.
pub fn extract_embedding(data: &[f32]) -> Vec<f32> {
    let v: Vec<f32> = data.iter().take(EMBEDDING_DIM).copied().collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm = if norm > 0.0 { norm } else { 1e-9 };
    v.into_iter().map(|x| x / norm).collect()
}

/// MiniFASNet-V2 3-class logits → p(live) (index 1).
pub fn softmax_live(logits: &[f32]) -> Option<f32> {
    if logits.len() < 3 {
        return None;
    }
    let m = logits[0].max(logits[1]).max(logits[2]);
    let e: Vec<f32> = logits[..3].iter().map(|x| (x - m).exp()).collect();
    let sum = e.iter().sum::<f32>();
    Some(e[1] / if sum > 0.0 { sum } else { 1e-9 })
}

/// Silent-Face CropImage.crop: scale the box by `margin` (capped to fit),
/// recentre, then shift (not clamp) into the frame.
pub fn antispoof_crop_rect(bbox: &BBox, w: f32, h: f32, margin: f32) -> BBox {
    let bw = (bbox[2] - bbox[0]).max(1.0);
    let bh = (bbox[3] - bbox[1]).max(1.0);
    let scale = margin.min((w - 1.0) / bw).min((h - 1.0) / bh);
    let (nw, nh) = (bw * scale, bh * scale);
    let cx = bbox[0] + bw / 2.0;
    let cy = bbox[1] + bh / 2.0;
    let (mut x1, mut y1, mut x2, mut y2) =
        (cx - nw / 2.0, cy - nh / 2.0, cx + nw / 2.0, cy + nh / 2.0);
    if x1 < 0.0 {
        x2 -= x1;
        x1 = 0.0;
    }
    if y1 < 0.0 {
        y2 -= y1;
        y1 = 0.0;
    }
    if x2 > w - 1.0 {
        x1 -= x2 - (w - 1.0);
        x2 = w - 1.0;
    }
    if y2 > h - 1.0 {
        y1 -= y2 - (h - 1.0);
        y2 = h - 1.0;
    }
    x1 = x1.clamp(0.0, w);
    y1 = y1.clamp(0.0, h);
    x2 = x2.max(0.0);
    y2 = y2.max(0.0);
    if x2 < x1 + 1.0 {
        x2 = x1 + 1.0;
    }
    if y2 < y1 + 1.0 {
        y2 = y1 + 1.0;
    }
    [x1, y1, x2, y2]
}

/// The 112×112 template SFace was trained on (OpenCV alignCrop).
pub const ARC_TEMPLATE: [Point; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

/// Least-squares similarity transform from the landmarks onto ARC_TEMPLATE,
/// as [a, b, c, d, e, f]: out_x = a·x + c·y + e, out_y = b·x + d·y + f.
pub fn align_transform(kps: &[Point; 5]) -> [f32; 6] {
    let n = 5.0;
    let (mut sxm, mut sym, mut dxm, mut dym) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for i in 0..5 {
        sxm += kps[i][0] / n;
        sym += kps[i][1] / n;
        dxm += ARC_TEMPLATE[i][0] / n;
        dym += ARC_TEMPLATE[i][1] / n;
    }
    let (mut a, mut b, mut sss) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..5 {
        let s0 = kps[i][0] - sxm;
        let s1 = kps[i][1] - sym;
        let d0 = ARC_TEMPLATE[i][0] - dxm;
        let d1 = ARC_TEMPLATE[i][1] - dym;
        a += s0 * d0 + s1 * d1;
        b += s0 * d1 - s1 * d0;
        sss += s0 * s0 + s1 * s1;
    }
    let p = a / sss;
    let q = b / sss;
    let t0 = dxm - (p * sxm - q * sym);
    let t1 = dym - (q * sxm + p * sym);
    [p, q, -q, p, t0, t1]
}

/// An RGB frame, 3 bytes per pixel, row-major.
#[derive(Clone)]
pub struct Rgb {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Rgb {
    /// Bilinear sample with pixel centres at +0.5 (canvas drawImage);
    /// outside the image is black.
    #[inline]
    fn sample(&self, x: f32, y: f32) -> [f32; 3] {
        let (w, h) = (self.width as i64, self.height as i64);
        let fx = x - 0.5;
        let fy = y - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let ax = fx - x0;
        let ay = fy - y0;
        let (x0, y0) = (x0 as i64, y0 as i64);
        let mut out = [0.0f32; 3];
        for (dy, wy) in [(0, 1.0 - ay), (1, ay)] {
            for (dx, wx) in [(0, 1.0 - ax), (1, ax)] {
                let wgt = wx * wy;
                if wgt == 0.0 {
                    continue;
                }
                // Clamp to the edge like a canvas does inside the drawn area.
                let px = (x0 + dx).clamp(0, w - 1);
                let py = (y0 + dy).clamp(0, h - 1);
                let i = ((py * w + px) * 3) as usize;
                out[0] += self.data[i] as f32 * wgt;
                out[1] += self.data[i + 1] as f32 * wgt;
                out[2] += self.data[i + 2] as f32 * wgt;
            }
        }
        out
    }

    fn inside(&self, x: f32, y: f32) -> bool {
        x >= 0.0 && y >= 0.0 && x < self.width as f32 && y < self.height as f32
    }
}

/// Planar [3, h, w] float tensor data, raw 0..255.
fn push_planar(buf: &mut [f32], plane: usize, i: usize, rgb: [f32; 3], bgr: bool) {
    let (first, last) = if bgr {
        (rgb[2], rgb[0])
    } else {
        (rgb[0], rgb[2])
    };
    buf[i] = first;
    buf[plane + i] = rgb[1];
    buf[2 * plane + i] = last;
}

/// Detector input: the frame resized to (sw, sh), zero-padded to (pw, ph),
/// BGR planar, raw 0..255.
pub fn detector_input(src: &Rgb, g: &Geometry) -> Vec<f32> {
    let plane = (g.pw * g.ph) as usize;
    let mut buf = vec![0.0f32; 3 * plane];
    let sx = src.width as f32 / g.sw as f32;
    let sy = src.height as f32 / g.sh as f32;
    for y in 0..g.sh {
        for x in 0..g.sw {
            let v = src.sample((x as f32 + 0.5) * sx, (y as f32 + 0.5) * sy);
            push_planar(&mut buf, plane, (y * g.pw + x) as usize, v, true);
        }
    }
    buf
}

/// Recognizer input: the frame warped by the alignment transform onto
/// 112×112, RGB planar, raw 0..255; pixels mapping outside the frame stay 0.
pub fn aligned_recognizer_input(src: &Rgb, kps: &[Point; 5]) -> Vec<f32> {
    let [a, b, c, d, e, f] = align_transform(kps);
    // Inverse of [[a c e][b d f]].
    let det = a * d - b * c;
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    let size = REC_SIZE as usize;
    let plane = size * size;
    let mut buf = vec![0.0f32; 3 * plane];
    for y in 0..size {
        for x in 0..size {
            let ox = x as f32 + 0.5 - e;
            let oy = y as f32 + 0.5 - f;
            let sx = ia * ox + ic * oy;
            let sy = ib * ox + id * oy;
            if !src.inside(sx, sy) {
                continue;
            }
            push_planar(&mut buf, plane, y * size + x, src.sample(sx, sy), false);
        }
    }
    buf
}

/// Anti-spoof input: the 2.7× crop resized to 80×80, BGR planar, raw 0..255.
pub fn antispoof_input(src: &Rgb, bbox: &BBox) -> Vec<f32> {
    let r = antispoof_crop_rect(bbox, src.width as f32, src.height as f32, ANTISPOOF_MARGIN);
    let size = ANTISPOOF_SIZE as usize;
    let plane = size * size;
    let mut buf = vec![0.0f32; 3 * plane];
    let sx = (r[2] - r[0]) / size as f32;
    let sy = (r[3] - r[1]) / size as f32;
    for y in 0..size {
        for x in 0..size {
            let v = src.sample(r[0] + (x as f32 + 0.5) * sx, r[1] + (y as f32 + 0.5) * sy);
            push_planar(&mut buf, plane, y * size + x, v, true);
        }
    }
    buf
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_matches_the_console() {
        let g = detector_geometry(1280, 720);
        assert_eq!(
            g,
            Geometry {
                scale: 0.5,
                sw: 640,
                sh: 360,
                pw: 640,
                ph: 384
            }
        );
        let g = detector_geometry(320, 240);
        assert_eq!((g.scale, g.pw, g.ph), (1.0, 320, 256));
    }

    #[test]
    fn alignment_of_the_template_is_identity() {
        let t = align_transform(&ARC_TEMPLATE);
        let expect = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        for (a, b) in t.iter().zip(expect) {
            assert!((a - b).abs() < 1e-4, "{t:?}");
        }
    }

    #[test]
    fn alignment_undoes_scale_and_shift() {
        let kps = ARC_TEMPLATE.map(|[x, y]| [x * 2.0 + 100.0, y * 2.0 + 50.0]);
        let [a, b, c, d, e, f] = align_transform(&kps);
        for (src, dst) in kps.iter().zip(ARC_TEMPLATE) {
            let ox = a * src[0] + c * src[1] + e;
            let oy = b * src[0] + d * src[1] + f;
            assert!((ox - dst[0]).abs() < 1e-3 && (oy - dst[1]).abs() < 1e-3);
        }
    }

    #[test]
    fn nms_keeps_the_best_of_overlapping_boxes() {
        let f = |x: f32, s: f32| RawFace {
            bbox: [x, 0.0, x + 10.0, 10.0],
            score: s,
            kps: [[0.0; 2]; 5],
        };
        let kept = nms(vec![f(0.0, 0.7), f(1.0, 0.9), f(50.0, 0.8)], NMS_IOU);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].score, 0.9);
    }

    #[test]
    fn softmax_reads_class_one_as_live() {
        let p = softmax_live(&[0.0, 10.0, 0.0]).unwrap();
        assert!(p > 0.99);
    }

    #[test]
    fn antispoof_crop_stays_inside_the_frame() {
        let r = antispoof_crop_rect(&[0.0, 0.0, 100.0, 100.0], 640.0, 480.0, ANTISPOOF_MARGIN);
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= 639.0 && r[3] <= 479.0);
        assert!(((r[2] - r[0]) - 270.0).abs() < 1e-3);
    }
}
