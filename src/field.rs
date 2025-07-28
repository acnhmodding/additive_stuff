use crate::data_type::DataType;

#[derive(Debug, Clone)]
pub struct Field {
    pub hash: u32,
    pub offset: i32,
    pub size: i32,
    pub data_type: DataType,
}
