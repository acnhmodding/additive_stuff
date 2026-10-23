/// A borrowed BCSV table; field types and widths come from its caller.
pub struct BcsvView<'a> {
    bytes: &'a [u8],
    pub count: usize,
    pub stride: usize,
    pub rows_start: usize,
    fields_start: usize,
}

impl<'a> BcsvView<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, &'static str> {
        if bytes.len() < 12 {
            return Err("Truncated BCSV header");
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let stride = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let field_count = u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as usize;
        let fields_start = match bytes[10] {
            0 => 12,
            1 if bytes.len() >= 28 && &bytes[12..16] == b"VSCB" => 28,
            _ => return Err("Invalid BCSV extended header"),
        };
        if stride < 4 || stride > i32::MAX as usize {
            return Err("Invalid BCSV row size");
        }
        let rows_start = fields_start + field_count * 8;
        let end = count
            .checked_mul(stride)
            .and_then(|n| rows_start.checked_add(n))
            .ok_or("BCSV size overflow")?;
        if end > bytes.len() {
            return Err("Truncated BCSV data");
        }
        let view = Self {
            bytes,
            count,
            stride,
            rows_start,
            fields_start,
        };
        if view.fields().any(|(_, offset)| offset >= stride) {
            return Err("Invalid BCSV field offset");
        }
        Ok(view)
    }

    /// The caller must provide a live native allocation containing the entire
    /// table described by its header. Header sizes are not allocation bounds.
    pub unsafe fn from_native(header: *const u8) -> Option<Self> {
        if header.is_null() {
            return None;
        }
        let prefix = std::slice::from_raw_parts(header, 12);
        let count = u32::from_le_bytes(prefix[0..4].try_into().ok()?) as usize;
        let stride = u32::from_le_bytes(prefix[4..8].try_into().ok()?) as usize;
        let fields = u16::from_le_bytes(prefix[8..10].try_into().ok()?) as usize;
        if count == 0
            || count > u16::MAX as usize
            || !(4..=0x10000).contains(&stride)
            || fields == 0
            || fields > 1024
            || prefix[10] > 1
        {
            return None;
        }
        let start = if prefix[10] == 1 { 28usize } else { 12usize } + fields * 8;
        let len = start.checked_add(count.checked_mul(stride)?)?;
        if len > u32::MAX as usize || (header as usize).checked_add(len).is_none() {
            return None;
        }
        Self::from_bytes(std::slice::from_raw_parts(header, len)).ok()
    }

    /// Read a scalar from a live native row whose first word is its file offset.
    /// The same allocation validity requirements as from_native apply.
    pub unsafe fn read_native_u32(row: *const u8, field_hash: u32) -> Option<u32> {
        if row.is_null() {
            return None;
        }
        let position = std::ptr::read_unaligned(row as *const u32) as usize;
        if position == 0 || position > row as usize {
            return None;
        }
        let table = Self::from_native(row.sub(position))?;
        if position < table.rows_start || (position - table.rows_start) % table.stride != 0 {
            return None;
        }
        let bytes = table.row((position - table.rows_start) / table.stride)?;
        let offset = table.field(field_hash, 4)?;
        Some(u32::from_le_bytes(
            bytes[offset..offset + 4].try_into().ok()?,
        ))
    }

    pub fn fields(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        self.bytes[self.fields_start..self.rows_start]
            .chunks_exact(8)
            .map(|field| {
                (
                    u32::from_le_bytes(field[0..4].try_into().unwrap()),
                    u32::from_le_bytes(field[4..8].try_into().unwrap()) as usize,
                )
            })
    }

    pub fn field(&self, hash: u32, width: usize) -> Option<usize> {
        let mut matches = self.fields().filter(|(candidate, _)| *candidate == hash);
        let (_, offset) = matches.next()?;
        if matches.next().is_some() || offset < 4 || offset.checked_add(width)? > self.stride {
            return None;
        }
        Some(offset)
    }

    pub fn row(&self, index: usize) -> Option<&'a [u8]> {
        if index >= self.count {
            return None;
        }
        let offset = self.rows_start + index * self.stride;
        let row = &self.bytes[offset..offset + self.stride];
        if u32::from_le_bytes(row[0..4].try_into().ok()?) as usize != offset {
            return None;
        }
        Some(row)
    }
}

pub fn string_field(row: &[u8], offset: usize, width: usize) -> Option<&[u8]> {
    let bytes = row.get(offset..offset.checked_add(width)?)?;
    let len = bytes.iter().position(|byte| *byte == 0)?;
    if len == 0 {
        return None;
    }
    Some(&bytes[..len])
}
