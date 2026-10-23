mod bcsv;
mod icon_registry;
mod menu_icons;
mod native_bcsv;
mod unit_icons;

#[skyline::main(name = "acnh_additive_stuff")]
pub fn main() {
    skyline::install_hooks!(
        unit_icons::build_icon_cache_hook,
        unit_icons::get_unit_icon_hook,
        unit_icons::collect_icon_resources_hook,
        menu_icons::build_menu_icon_cache_hook,
        menu_icons::get_menu_icon_hook
    );
}
