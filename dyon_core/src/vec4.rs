use super::*;

/// Wraps a 4D vector for easier embedding with Dyon.
#[derive(Debug, Copy, Clone)]
pub struct Vec4(pub [f32; 4]);

/// Implemented by types that can be converted to and from vec4.
pub trait ConvertVec4: Sized {
    /// Converts vec4 to self.
    fn from(val: [f32; 4]) -> Self;
    /// Converts from self to vec4.
    fn to(&self) -> [f32; 4];
}

impl ConvertVec4 for Vec4 {
    fn from(val: [f32; 4]) -> Self { Vec4(val) }
    fn to(&self) -> [f32; 4] { self.0 }
}

impl ConvertVec4 for [f32; 2] {
    fn from(val: [f32; 4]) -> Self { [val[0], val[1]] }
    fn to(&self) -> [f32; 4] { [self[0], self[1], 0.0, 0.0] }
}

impl ConvertVec4 for [f32; 3] {
    fn from(val: [f32; 4]) -> Self { [val[0], val[1], val[2]] }
    fn to(&self) -> [f32; 4] { [self[0], self[1], self[2], 0.0] }
}

impl ConvertVec4 for [f32; 4] {
    fn from(val: [f32; 4]) -> Self { val }
    fn to(&self) -> [f32; 4] { *self }
}

impl ConvertVec4 for [f64; 2] {
    fn from(val: [f32; 4]) -> Self { [f64::from(val[0]), f64::from(val[1])] }
    fn to(&self) -> [f32; 4] { [self[0] as f32, self[1] as f32, 0.0, 0.0] }
}

impl ConvertVec4 for [f64; 3] {
    fn from(val: [f32; 4]) -> Self {
        [f64::from(val[0]), f64::from(val[1]), f64::from(val[2])]
    }
    fn to(&self) -> [f32; 4] { [self[0] as f32, self[1] as f32, self[2] as f32, 0.0] }
}

impl ConvertVec4 for [f64; 4] {
    fn from(val: [f32; 4]) -> Self {
        [f64::from(val[0]), f64::from(val[1]), f64::from(val[2]), f64::from(val[3])]
    }
    fn to(&self) -> [f32; 4] { [self[0] as f32, self[1] as f32, self[2] as f32, self[3] as f32] }
}

impl<R, V> PopVariable<R> for Vec4
    where Self: VariableType<R, Variable = V>,
          R: RuntimeErrorHandling<Variable = V>,
          V: VariableCore
{
    fn pop_var(rt: &R, var: &V) -> Result<Self, String> {
        if let Some(v) = var.get_vec4() {
            Ok(Vec4(*v))
        } else {
            Err(rt.expected(var, "vec4"))
        }
    }
}

impl<R, V> PushVariable<R> for Vec4
    where Self: VariableType<R, Variable = V>,
          V: VariableCore
{
    fn push_var(&self) -> V { V::vec4(self.0) }
}

impl From<[f32; 2]> for Vec4 {
    fn from(val: [f32; 2]) -> Vec4 {
        Vec4([val[0], val[1], 0.0, 0.0])
    }
}

impl From<[f32; 3]> for Vec4 {
    fn from(val: [f32; 3]) -> Vec4 {
        Vec4([val[0], val[1], val[2], 0.0])
    }
}

impl From<[f32; 4]> for Vec4 {
    fn from(val: [f32; 4]) -> Vec4 {
        Vec4([val[0], val[1], val[2], val[3]])
    }
}

impl From<[f64; 2]> for Vec4 {
    fn from(val: [f64; 2]) -> Vec4 {
        Vec4([val[0] as f32, val[1] as f32, 0.0, 0.0])
    }
}

impl From<[f64; 3]> for Vec4 {
    fn from(val: [f64; 3]) -> Vec4 {
        Vec4([val[0] as f32, val[1] as f32, val[2] as f32, 0.0])
    }
}

impl From<[f64; 4]> for Vec4 {
    fn from(val: [f64; 4]) -> Vec4 {
        Vec4([val[0] as f32, val[1] as f32, val[2] as f32, val[3] as f32])
    }
}

impl From<(f32, f32)> for Vec4 {
    fn from(val: (f32, f32)) -> Vec4 {
        Vec4([val.0, val.1, 0.0, 0.0])
    }
}

impl From<(f32, f32, f32)> for Vec4 {
    fn from(val: (f32, f32, f32)) -> Vec4 {
        Vec4([val.0, val.1, val.2, 0.0])
    }
}

impl From<(f32, f32, f32, f32)> for Vec4 {
    fn from(val: (f32, f32, f32, f32)) -> Vec4 {
        Vec4([val.0, val.1, val.2, val.3])
    }
}

impl From<(f64, f64)> for Vec4 {
    fn from(val: (f64, f64)) -> Vec4 {
        Vec4([val.0 as f32, val.1 as f32, 0.0, 0.0])
    }
}

impl From<(f64, f64, f64)> for Vec4 {
    fn from(val: (f64, f64, f64)) -> Vec4 {
        Vec4([val.0 as f32, val.1 as f32, val.2 as f32, 0.0])
    }
}

impl From<(f64, f64, f64, f64)> for Vec4 {
    fn from(val: (f64, f64, f64, f64)) -> Vec4 {
        Vec4([val.0 as f32, val.1 as f32, val.2 as f32, val.3 as f32])
    }
}

impl From<[u32; 2]> for Vec4 {
    fn from(val: [u32; 2]) -> Vec4 {
        Vec4([val[0] as f32, val[1] as f32, 0.0, 0.0])
    }
}

impl From<(u32, u32)> for Vec4 {
    fn from(val: (u32, u32)) -> Vec4 {
        Vec4([val.0 as f32, val.1 as f32, 0.0, 0.0])
    }
}

impl From<[u8; 4]> for Vec4 {
    fn from(val: [u8; 4]) -> Vec4 {
        Vec4([f32::from(val[0]) / 255.0, f32::from(val[1]) / 255.0,
              f32::from(val[2]) / 255.0, f32::from(val[3]) / 255.0])
    }
}

impl From<(u8, u8, u8, u8)> for Vec4 {
    fn from(val: (u8, u8, u8, u8)) -> Vec4 {
        Vec4([f32::from(val.0) / 255.0, f32::from(val.1) / 255.0,
              f32::from(val.2) / 255.0, f32::from(val.3) / 255.0])
    }
}

impl Into<[f32; 2]> for Vec4 {
    fn into(self) -> [f32; 2] {
        [self.0[0], self.0[1]]
    }
}

impl Into<[f32; 3]> for Vec4 {
    fn into(self) -> [f32; 3] {
        [self.0[0], self.0[1], self.0[2]]
    }
}

impl Into<[f32; 4]> for Vec4 {
    fn into(self) -> [f32; 4] {
        self.0
    }
}

impl Into<[f64; 2]> for Vec4 {
    fn into(self) -> [f64; 2] {
        [f64::from(self.0[0]), f64::from(self.0[1])]
    }
}

impl Into<[f64; 3]> for Vec4 {
    fn into(self) -> [f64; 3] {
        [f64::from(self.0[0]), f64::from(self.0[1]), f64::from(self.0[2])]
    }
}

impl Into<[f64; 4]> for Vec4 {
    fn into(self) -> [f64; 4] {
        [f64::from(self.0[0]), f64::from(self.0[1]), f64::from(self.0[2]), f64::from(self.0[3])]
    }
}

impl Into<(f32, f32)> for Vec4 {
    fn into(self) -> (f32, f32) {
        (self.0[0], self.0[1])
    }
}

impl Into<(f32, f32, f32)> for Vec4 {
    fn into(self) -> (f32, f32, f32) {
        (self.0[0], self.0[1], self.0[2])
    }
}

impl Into<(f32, f32, f32, f32)> for Vec4 {
    fn into(self) -> (f32, f32, f32, f32) {
        (self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

impl Into<(f64, f64)> for Vec4 {
    fn into(self) -> (f64, f64) {
        (f64::from(self.0[0]), f64::from(self.0[1]))
    }
}

impl Into<(f64, f64, f64)> for Vec4 {
    fn into(self) -> (f64, f64, f64) {
        (f64::from(self.0[0]), f64::from(self.0[1]), f64::from(self.0[2]))
    }
}

impl Into<(f64, f64, f64, f64)> for Vec4 {
    fn into(self) -> (f64, f64, f64, f64) {
        (f64::from(self.0[0]), f64::from(self.0[1]), f64::from(self.0[2]), f64::from(self.0[3]))
    }
}

impl Into<[u32; 2]> for Vec4 {
    fn into(self) -> [u32; 2] {
        [self.0[0] as u32, self.0[1] as u32]
    }
}

impl Into<(u32, u32)> for Vec4 {
    fn into(self) -> (u32, u32) {
        (self.0[0] as u32, self.0[1] as u32)
    }
}

impl Into<(u8, u8, u8, u8)> for Vec4 {
    fn into(self) -> (u8, u8, u8, u8) {
        ((self.0[0] * 255.0) as u8, (self.0[1] * 255.0) as u8,
         (self.0[2] * 255.0) as u8, (self.0[3] * 255.0) as u8)
    }
}

impl Into<[u8; 4]> for Vec4 {
    fn into(self) -> [u8; 4] {
        [(self.0[0] * 255.0) as u8, (self.0[1] * 255.0) as u8,
         (self.0[2] * 255.0) as u8, (self.0[3] * 255.0) as u8]
    }
}
