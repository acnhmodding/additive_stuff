#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    BitField,
    S8,
    U8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    CRC32,
    MMH3,
    Float32,
    Float64,
    String,
}