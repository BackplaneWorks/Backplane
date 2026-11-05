//! Generated from the existing web theme, rather than a second palette.
include!(concat!(env!("OUT_DIR"), "/theme.rs"));
pub fn color(light: bool, name:&str)->gpui::Rgba {
    let find=|palette:&[(&str,u32)]|palette.iter().find(|(k,_)|*k==name).map(|(_,v)|*v);
    // A token the light block does not redefine inherits :root, as in CSS.
    let value=if light {find(LIGHT).or_else(||find(DARK))} else {find(DARK)};
    gpui::rgb(value.unwrap_or(0xff00ff))
}
