//! `SexyMatrix3`: a 2D affine transform in screen space (y down).

use std::ops::Mul;

/// Maps (x, y) to (m00·x + m01·y + m02, m10·x + m11·y + m12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3 {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
}

impl Matrix3 {
    pub const IDENTITY: Self = Self { m00: 1.0, m01: 0.0, m02: 0.0, m10: 0.0, m11: 1.0, m12: 0.0 };

    pub fn translation(x: f32, y: f32) -> Self {
        Self { m02: x, m12: y, ..Self::IDENTITY }
    }

    /// `SexyMatrix3Translation`: moves the result by (x, y).
    pub fn translated(self, x: f32, y: f32) -> Self {
        Self { m02: self.m02 + x, m12: self.m12 + y, ..self }
    }

    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.m00 * x + self.m01 * y + self.m02, self.m10 * x + self.m11 * y + self.m12)
    }
}

impl Default for Matrix3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// `a * b` applies `b` first, like `SexyMatrix3Multiply(result, a, b)`.
impl Mul for Matrix3 {
    type Output = Self;

    fn mul(self, b: Self) -> Self {
        let a = self;
        Self {
            m00: a.m00 * b.m00 + a.m01 * b.m10,
            m01: a.m00 * b.m01 + a.m01 * b.m11,
            m02: a.m00 * b.m02 + a.m01 * b.m12 + a.m02,
            m10: a.m10 * b.m00 + a.m11 * b.m10,
            m11: a.m10 * b.m01 + a.m11 * b.m11,
            m12: a.m10 * b.m02 + a.m11 * b.m12 + a.m12,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiply_applies_right_first() {
        let scale = Matrix3 { m00: 2.0, m11: 2.0, ..Matrix3::IDENTITY };
        let m = Matrix3::translation(10.0, 0.0) * scale;
        assert_eq!(m.transform_point(1.0, 1.0), (12.0, 2.0));
    }
}
