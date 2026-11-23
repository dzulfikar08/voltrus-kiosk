//! The three ONNX sessions and the pipeline of console faceEngine.ts:
//! detect (YuNet) → best face → align → embed (SFace int8) → liveness
//! (MiniFASNet-V2).

use super::math::{self, RawFace, Rgb};
use anyhow::{Context as _, Result, anyhow};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;
use std::path::Path;

pub struct Engine {
    det: Session,
    rec: Session,
    antispoof: Option<Session>,
}

#[derive(Debug, Clone)]
pub struct Embedded {
    pub face: RawFace,
    pub embedding: Vec<f32>,
    /// p(live), None when the anti-spoofing model is missing or failed.
    pub liveness: Option<f32>,
}

fn session(path: &Path) -> Result<Session> {
    Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(2)?
        // ONNX Runtime's pool threads busy-wait between runs by default,
        // which kept a core warm between frames for nothing.
        .with_intra_op_spinning(false)?
        .with_inter_op_spinning(false)?
        .commit_from_file(path)
        .with_context(|| format!("loading {}", path.display()))
}

impl Engine {
    pub fn load(dir: &Path) -> Result<Self> {
        let antispoof = dir.join("antispoof.onnx");
        Ok(Self {
            det: session(&dir.join("face_det.onnx"))?,
            rec: session(&dir.join("face_rec.onnx"))?,
            antispoof: if antispoof.exists() {
                Some(session(&antispoof)?)
            } else {
                None
            },
        })
    }

    pub fn detect(&mut self, frame: &Rgb) -> Result<Vec<RawFace>> {
        let g = math::detector_geometry(frame.width, frame.height);
        let input = math::detector_input(frame, &g);
        let tensor = Tensor::from_array(([1usize, 3, g.ph as usize, g.pw as usize], input))?;
        let name = self.det.inputs[0].name.clone();
        let out = self.det.run(ort::inputs![name => tensor])?;
        let mut owned: Vec<(u32, [Vec<f32>; 4])> = Vec::new();
        for stride in math::STRIDES {
            let read = |key: &str| -> Result<Vec<f32>> {
                let v = out
                    .get(key)
                    .ok_or_else(|| anyhow!("detector output {key} missing"))?;
                Ok(v.try_extract_tensor::<f32>()?.1.to_vec())
            };
            owned.push((
                stride,
                [
                    read(&format!("cls_{stride}"))?,
                    read(&format!("obj_{stride}"))?,
                    read(&format!("bbox_{stride}"))?,
                    read(&format!("kps_{stride}"))?,
                ],
            ));
        }
        Ok(math::decode_yunet(
            |stride| {
                let (_, [cls, obj, bbox, kps]) = owned
                    .iter()
                    .find(|(s, _)| *s == stride)
                    .expect("every stride decoded");
                math::YunetOutputs {
                    cls,
                    obj,
                    bbox,
                    kps,
                }
            },
            g.pw,
            g.ph,
            g.scale,
            math::DET_THRESH,
        ))
    }

    pub fn embed_face(&mut self, frame: &Rgb, face: &RawFace) -> Result<Vec<f32>> {
        let input = math::aligned_recognizer_input(frame, &face.kps);
        let s = math::REC_SIZE as usize;
        let tensor = Tensor::from_array(([1usize, 3, s, s], input))?;
        let name = self.rec.inputs[0].name.clone();
        let out = self.rec.run(ort::inputs![name => tensor])?;
        let (_, data) = out[0].try_extract_tensor::<f32>()?;
        Ok(math::extract_embedding(data))
    }

    pub fn liveness(&mut self, frame: &Rgb, face: &RawFace) -> Option<f32> {
        let session = self.antispoof.as_mut()?;
        let input = math::antispoof_input(frame, &face.bbox);
        let s = math::ANTISPOOF_SIZE as usize;
        let tensor = Tensor::from_array(([1usize, 3, s, s], input)).ok()?;
        let name = session.inputs[0].name.clone();
        let out = session.run(ort::inputs![name => tensor]).ok()?;
        let (_, logits) = out[0].try_extract_tensor::<f32>().ok()?;
        math::softmax_live(logits)
    }

    /// The largest detected face, embedded, with its liveness.
    pub fn embed(&mut self, frame: &Rgb) -> Result<Option<Embedded>> {
        let faces = self.detect(frame)?;
        let Some(face) = faces
            .into_iter()
            .max_by(|a, b| area(&a.bbox).total_cmp(&area(&b.bbox)))
        else {
            return Ok(None);
        };
        let embedding = self.embed_face(frame, &face)?;
        let liveness = self.liveness(frame, &face);
        Ok(Some(Embedded {
            face,
            embedding,
            liveness,
        }))
    }

    pub fn has_antispoof(&self) -> bool {
        self.antispoof.is_some()
    }
}

fn area(b: &math::BBox) -> f32 {
    (b[2] - b[0]) * (b[3] - b[1])
}
