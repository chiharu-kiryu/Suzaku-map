//! Resolution-independent geometry, shared by the GPU panel and CPU tray rasterizer.
use super::{CandidateQuad, QuadShape};

impl CandidateQuad {
    pub(crate) fn translate(&mut self, offset: [f32; 2]) {
        self.rect[0] += offset[0];
        self.rect[1] += offset[1];
        if let Some(clip) = &mut self.clip_rect {
            clip[0] += offset[0];
            clip[1] += offset[1];
        }
        if let QuadShape::Segment { start, end, .. } = &mut self.shape {
            for axis in 0..2 {
                start[axis] += offset[axis];
                end[axis] += offset[axis];
            }
        }
    }

    pub fn rounded(rect: [f32; 4], color: [f32; 4], radius: f32) -> Self {
        Self {
            rect,
            color,
            shape: QuadShape::Rounded {
                radius: radius.max(0.0).min(rect[2].min(rect[3]).max(0.0) * 0.5),
                stroke: 0.0,
            },
            clip_rect: None,
        }
    }

    pub fn outline(rect: [f32; 4], color: [f32; 4], radius: f32, stroke: f32) -> Self {
        let mut quad = Self::rounded(rect, color, radius);
        if let QuadShape::Rounded { stroke: width, .. } = &mut quad.shape {
            *width = stroke.max(0.0).min(rect[2].min(rect[3]).max(0.0) * 0.5);
        }
        quad
    }

    pub fn line(start: [f32; 2], end: [f32; 2], color: [f32; 4], width: f32) -> Self {
        let radius = width.max(0.0) * 0.5;
        Self {
            rect: [
                start[0].min(end[0]) - radius,
                start[1].min(end[1]) - radius,
                (end[0] - start[0]).abs() + radius * 2.0,
                (end[1] - start[1]).abs() + radius * 2.0,
            ],
            color,
            shape: QuadShape::Segment { start, end, radius },
            clip_rect: None,
        }
    }

    /// Two vec4 attributes consumed by panel/shapes.wgsl. Coordinates stay in scene pixels.
    pub fn shape_parameters(&self) -> ([f32; 4], [f32; 4]) {
        let [x, y, w, h] = self.rect;
        match self.shape {
            QuadShape::Rectangle => ([x + w * 0.5, y + h * 0.5, w * 0.5, h * 0.5], [0.0; 4]),
            QuadShape::Rounded { radius, stroke } => (
                [x + w * 0.5, y + h * 0.5, w * 0.5, h * 0.5],
                [radius, stroke, 1.0, 0.0],
            ),
            QuadShape::Segment { start, end, radius } => (
                [start[0], start[1], end[0], end[1]],
                [radius, 0.0, 2.0, 0.0],
            ),
        }
    }

    pub fn signed_distance(&self, point: [f32; 2]) -> f32 {
        let (geometry, style) = self.shape_parameters();
        if let QuadShape::Segment { start, end, radius } = self.shape {
            let delta = [end[0] - start[0], end[1] - start[1]];
            let p = [point[0] - start[0], point[1] - start[1]];
            let t = ((p[0] * delta[0] + p[1] * delta[1])
                / (delta[0] * delta[0] + delta[1] * delta[1]).max(0.0001))
            .clamp(0.0, 1.0);
            return (p[0] - delta[0] * t).hypot(p[1] - delta[1] * t) - radius;
        }
        let q = [
            (point[0] - geometry[0]).abs() - geometry[2] + style[0],
            (point[1] - geometry[1]).abs() - geometry[3] + style[0],
        ];
        let distance = q[0].max(0.0).hypot(q[1].max(0.0)) + q[0].max(q[1]).min(0.0) - style[0];
        if style[1] > 0.0 {
            (distance + style[1] * 0.5).abs() - style[1] * 0.5
        } else {
            distance
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translating_a_stroke_moves_its_curve_and_clip_with_its_bounds() {
        let mut line = CandidateQuad::line([2.0, 3.0], [8.0, 9.0], [1.0; 4], 2.0);
        line.clip_rect = Some([0.0, 0.0, 10.0, 10.0]);
        let distance = line.signed_distance([5.0, 6.0]);
        line.translate([20.0, 30.0]);
        assert_eq!(line.signed_distance([25.0, 36.0]), distance);
        assert_eq!(line.clip_rect, Some([20.0, 30.0, 10.0, 10.0]));
    }

    #[test]
    fn circles_and_strokes_have_true_curved_boundaries() {
        let circle = CandidateQuad::rounded([0.0, 0.0, 20.0, 20.0], [1.0; 4], 10.0);
        assert!(circle.signed_distance([1.0, 1.0]) > 0.0);
        assert!(circle.signed_distance([10.0, 10.0]) < -9.0);
        assert!(circle.signed_distance([10.0, 0.0]).abs() < 0.001);
        let ring = CandidateQuad::outline(circle.rect, circle.color, 10.0, 2.0);
        assert!(ring.signed_distance([10.0, 10.0]) > 0.0);
        assert!(ring.signed_distance([10.0, 1.0]) < 0.0);
    }

    #[test]
    fn diagonal_strokes_are_capsules_including_zero_length() {
        let line = CandidateQuad::line([4.0, 4.0], [16.0, 16.0], [1.0; 4], 2.0);
        assert!(line.signed_distance([10.0, 10.0]) < 0.0);
        assert!(line.signed_distance([4.0, 16.0]) > 5.0);
        assert!(line.signed_distance([3.5, 3.5]) < 0.0);
        let dot = CandidateQuad::line([4.0; 2], [4.0; 2], [1.0; 4], 2.0);
        assert_eq!(dot.signed_distance([4.0; 2]), -1.0);
    }
}
