use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::bcsv::{crc32, NativeU32Field, StringField};
use crate::native_bcsv::{BcsvCache, BorrowedString};

#[repr(C)]
pub struct ItemData {
    reserved_000: [u8; 0x10],
    item_param_row: *const u8,
}

const LABEL: StringField = StringField::new(b"Label string32", 32);

pub struct IconSpec {
    pub filename: &'static [u8],
    // Hash of the ItemParam field holding the requested icon Label hash.
    pub item_field: u32,
    pub model_name: Option<StringField>,
    pub res_name: StringField,
    // MenuIcons are named UI requests; only UnitIcons use the preload list.
    pub preload_resources: bool,
}

pub struct IconRegistry {
    indices_by_label_hash: HashMap<u32, u16>,
    pub resource_names: Vec<CString>,
}

struct IconState {
    // None blocks custom lookups during rebuilds or after extension failure.
    registry: Option<Arc<IconRegistry>>,
    item_field: Option<NativeU32Field>,
}

pub struct IconTable {
    spec: IconSpec,
    state: Mutex<IconState>,
    warned: AtomicBool,
}

impl IconTable {
    pub const fn new(spec: IconSpec) -> Self {
        Self {
            spec,
            state: Mutex::new(IconState {
                registry: None,
                item_field: None,
            }),
            warned: AtomicBool::new(false),
        }
    }

    pub unsafe fn begin_build(&self, path: *const BorrowedString) -> bool {
        let Some(path) = path.as_ref().filter(|path| !path.text.is_null()) else {
            return false;
        };
        let relevant = CStr::from_ptr(path.text.cast())
            .to_bytes()
            .rsplit(|&byte| byte == b'/')
            .next()
            == Some(self.spec.filename);
        if relevant {
            if let Ok(mut state) = self.state.lock() {
                state.registry = None;
            }
        }
        relevant
    }

    pub unsafe fn finish_build(&self, cache: *mut BcsvCache) {
        let Some(native) = cache.as_mut() else { return };
        match self.extend_cache(native) {
            Ok(registry) => {
                if let Ok(mut state) = self.state.lock() {
                    state.registry = Some(registry);
                }
            }
            Err(message) => {
                if !self.warned.swap(true, Ordering::Relaxed) {
                    println!(
                        "[additive icons] {}: {}",
                        String::from_utf8_lossy(self.spec.filename),
                        message
                    );
                }
            }
        }
    }

    pub fn current(&self) -> Option<Arc<IconRegistry>> {
        self.state.lock().ok()?.registry.clone()
    }

    pub unsafe fn index_for(&self, item: *const ItemData) -> Option<u16> {
        let row = item.as_ref()?.item_param_row;
        let mut state = self.state.lock().ok()?;
        if state.registry.as_ref()?.indices_by_label_hash.is_empty() {
            return None;
        }
        let hash = match state.item_field.and_then(|field| field.read(row)) {
            Some(hash) => hash,
            None => {
                let field = NativeU32Field::resolve(row, self.spec.item_field)?;
                state.item_field = Some(field);
                field.read(row)?
            }
        };
        state
            .registry
            .as_ref()?
            .indices_by_label_hash
            .get(&hash)
            .copied()
    }

    unsafe fn extend_cache(
        &self,
        native: &mut BcsvCache,
    ) -> Result<Arc<IconRegistry>, &'static str> {
        let mut cache = native.extension()?;
        let table = cache.table;
        let label_column = table.column(LABEL).ok_or("missing Label descriptor")?;
        let model_column = match self.spec.model_name {
            Some(field) => Some(table.column(field).ok_or("missing ModelName descriptor")?),
            None => None,
        };
        let resource_column = table
            .column(self.spec.res_name)
            .ok_or("missing ResName descriptor")?;
        let mut indices_by_label_hash = HashMap::new();
        let mut resource_names = Vec::new();
        let mut new_rows = Vec::new();
        let mut labels = HashMap::new();
        let mut resources = HashSet::new();

        for raw_index in 0..table.count() {
            if !cache.is_valid(raw_index) {
                continue;
            }
            let row = table.row(raw_index).ok_or("invalid native row position")?;
            let label = label_column.read(row).ok_or("invalid icon Label")?;
            let hash = crc32(label.to_bytes());
            let vanilla = cache.is_mapped(raw_index)?;
            let (existing_label, existing_is_vanilla) =
                labels.entry(hash).or_insert((label, vanilla));
            if *existing_label != label {
                return Err("ambiguous icon Label CRC collision");
            }
            if *existing_is_vanilla != vanilla {
                return Err("custom Label would override an existing native icon");
            }
            if vanilla {
                continue;
            }

            if let Some(column) = model_column {
                column.read(row).ok_or("invalid custom ModelName")?;
            }
            let resource = resource_column.read(row).ok_or("invalid custom ResName")?;
            if resource.to_bytes().len() > 31 {
                return Err("custom ResName exceeds the renderer's 31-byte limit");
            }
            // Identical duplicate Labels use their first assigned icon index.
            indices_by_label_hash
                .entry(hash)
                .or_insert(cache.next_alias(new_rows.len()));
            new_rows.push((raw_index, row.as_ptr()));
            if self.spec.preload_resources && resources.insert(resource) {
                resource_names.push(resource.to_owned());
            }
        }
        // Validate everything before writing native arrays; publish count last.
        cache.append(&new_rows)?;
        Ok(Arc::new(IconRegistry {
            indices_by_label_hash,
            resource_names,
        }))
    }
}

/// Defines the cache-build and icon-selector hooks shared by every icon kind.
macro_rules! icon_hooks {
    ($table:ident, $build_hook:ident = $build:ident, $get_hook:ident = $get:ident) => {
        #[skyline::hook(offset = $build)]
        pub unsafe fn $build_hook(
            cache: *mut $crate::native_bcsv::BcsvCache,
            path: *const $crate::native_bcsv::BorrowedString,
            allocator: *mut u8,
        ) {
            let relevant = $table.begin_build(path);
            call_original!(cache, path, allocator);
            if relevant {
                $table.finish_build(cache);
            }
        }

        #[skyline::hook(offset = $get)]
        pub unsafe fn $get_hook(item: *const $crate::icon_registry::ItemData) -> u16 {
            $table
                .index_for(item)
                .unwrap_or_else(|| call_original!(item))
        }
    };
}
pub(crate) use icon_hooks;

const _: () = {
    assert!(std::mem::size_of::<ItemData>() == 0x18);
    assert!(std::mem::offset_of!(ItemData, item_param_row) == 0x10);
};
