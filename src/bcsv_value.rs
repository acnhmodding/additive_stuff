#[derive(Debug, Clone)]
pub enum BCSVValue {
    Byte(u8),
    SByte(i8),
    Short(i16),
    UShort(u16),
    Int(i32),
    UInt(u32),
    Float(f32),
    Double(f64),
    String(String),
    Bytes(Vec<u8>),
}