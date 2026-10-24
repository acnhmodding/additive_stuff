mod bcsv;
mod icon_registry;
mod icons;
mod native_bcsv;

#[skyline::main(name = "acnh_additive_stuff")]
pub fn main() {
    skyline::install_hooks!(
        icons::build_unit_icon_cache_hook,
        icons::get_unit_icon_hook,
        icons::collect_icon_resources_hook,
        icons::build_menu_icon_cache_hook,
        icons::get_menu_icon_hook
    );
}
