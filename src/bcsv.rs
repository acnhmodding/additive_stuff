use std::io::{Cursor, Seek, SeekFrom, Read};
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{data_type::DataType, field::Field, bcsv_value::BCSVValue};

#[derive(Debug)]
pub struct BCSV {
    pub entry_size: i32,
    pub fields: Vec<Field>,
    pub entries: Vec<Vec<BCSVValue>>,
    pub has_extended_header: u8,
    pub unknown_field: u8,
    pub header_version: i32,
}

impl BCSV {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = Cursor::new(bytes);

        let entry_count = reader.read_u32::<LittleEndian>()?;
        let entry_size = reader.read_i32::<LittleEndian>()?;
        let field_count = reader.read_u16::<LittleEndian>()?;

        let has_extended_header = reader.read_u8()?;
        let unknown_field = reader.read_u8()?;

        let mut header_version = 0;
        if has_extended_header == 1 {
            let mut magic: [u8; 4]  = [0u8; 4];
            reader.read_exact(&mut magic)?;
            if &magic != b"VSCB" {
                return Err("Invalid BCSV magic".into());
            }

            header_version = reader.read_i32::<LittleEndian>()?;
            reader.seek(SeekFrom::Current(8))?;
        }

        // Read field definitions
        let mut fields = Vec::with_capacity(field_count as usize);
        for _ in 0..field_count {
            let hash = reader.read_u32::<LittleEndian>()?;
            let offset = reader.read_i32::<LittleEndian>()?;
            fields.push(Field {
                hash,
                offset,
                size: 0,
                data_type: DataType::UInt32,
            });
        }

        // Compute field sizes and assign dummy types
        for i in 0..fields.len() {
            let end = if i + 1 < fields.len() {
                fields[i + 1].offset
            } else {
                entry_size
            };
            
            fields[i].size = end - fields[i].offset;
            // fields[i].data_type = match fields[i].size {
            //     1 => DataType::U8,
            //     2 => DataType::UInt16,
            //     4 => DataType::UInt32,
            //     _ => DataType::BitField,
            // };
            fields[i].data_type = DataType::String;
        }

        // Read entries
        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let entry_pos = reader.read_u32::<LittleEndian>()?;
            let mut entry = Vec::with_capacity(fields.len());

            for field in &fields {
                reader.seek(SeekFrom::Start(entry_pos as u64 + field.offset as u64))?;
                let value = match field.data_type {
                    DataType::BitField => {
                        let mut buf = vec![0u8; field.size as usize];
                        reader.read_exact(&mut buf)?;
                        BCSVValue::Bytes(buf)
                    }
                    DataType::S8 => BCSVValue::SByte(reader.read_i8()?),
                    DataType::U8 => BCSVValue::Byte(reader.read_u8()?),
                    DataType::Int16 => BCSVValue::Short(reader.read_i16::<LittleEndian>()?),
                    DataType::UInt16 => BCSVValue::UShort(reader.read_u16::<LittleEndian>()?),
                    DataType::Int32 => BCSVValue::Int(reader.read_i32::<LittleEndian>()?),
                    DataType::UInt32 | DataType::CRC32 | DataType::MMH3 => {
                        BCSVValue::UInt(reader.read_u32::<LittleEndian>()?)
                    }
                    DataType::Float32 => BCSVValue::Float(reader.read_f32::<LittleEndian>()?),
                    DataType::Float64 => BCSVValue::Double(reader.read_f64::<LittleEndian>()?),
                    DataType::String => {
                        let mut buf = vec![0u8; field.size as usize];
                        reader.read_exact(&mut buf)?;
                        let str_end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                        let str_val = String::from_utf8_lossy(&buf[..str_end]).to_string();
                        BCSVValue::String(str_val)
                    }
                };
                entry.push(value);
            }

            entries.push(entry);
            reader.seek(SeekFrom::Start(entry_pos as u64 + entry_size as u64))?;
        }

        Ok(Self {
            entry_size,
            fields,
            entries,
            has_extended_header,
            unknown_field,
            header_version,
        })
    }
}
