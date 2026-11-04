//! Generated from the existing web theme, rather than a second palette.
include!(concat!(env!("OUT_DIR"), "/theme.rs"));
pub fn color(light: bool, name:&str)->gpui::Rgba {
    let palette=if light {LIGHT} else {DARK};
    gpui::rgb(palette.iter().find(|(k,_)|*k==name).expect("theme token").1)
}
