//! Generated from the existing web theme, rather than a second palette.
/* i18n-ok: css parsing and build checks */ include!(concat!(env!("OUT_DIR"), "/theme.rs"));
pub fn color(light: bool, name:&str)->gpui::Rgba {
    let find=|palette:&[(&str,u32)]|palette.iter().find(|(k,_)|*k==name).map(|(_,v)|*v);
    // A token the light block does not redefine inherits :root, as in CSS.
    let value=if light {find(LIGHT).or_else(||find(DARK))} else {find(DARK)};
    gpui::rgb(value.unwrap_or(0xff00ff))
}

/// Every token of a theme as the page resolves it: the light block over :root.
pub fn tokens(light: bool) -> Vec<(&'static str, u32)> {
    DARK.iter().map(|(k, d)| (*k, if light { LIGHT.iter().find(|(n, _)| n == k).map_or(*d, |(_, v)| *v) } else { *d })).collect()
}
#[cfg(test)] mod tests {
    use super::*;
    /// Independent parser of src/web/style.css: every `--token: #rrggbb` of the two theme blocks must be in theme.rs exactly.
    fn block(css: &str, selector: &str) -> Vec<(String, u32)> {
        let body = css.split(selector).nth(1).unwrap().split('}').next().unwrap();
        body.lines().filter_map(|l| {
            let (k, v) = l.trim().strip_prefix("--")?.split_once(':')?;
            let v = v.trim().trim_end_matches(';');
            let hex = v.strip_prefix('#')?;
            (hex.len() == 6).then(|| (k.to_string(), u32::from_str_radix(hex, 16).unwrap()))
        }).collect()
    }
    #[test] fn tokens_equal_the_css() {
        let css = include_str!("../../../assets/web/style.css");
        /* i18n-ok: css parsing and build checks */ for (selector, palette, light) in [(":root {", DARK, false), ("html.light, .app.light, .bub-move.light {", LIGHT, true)] {
            let want = block(css, selector);
            /* i18n-ok: css parsing and build checks */ assert!(want.len() >= 17, "{selector}: parsed {} tokens", want.len());
            /* i18n-ok: css parsing and build checks */ for (k, v) in &want { assert_eq!(palette.iter().find(|(n, _)| n == k).map(|(_, v)| *v), Some(*v), "{selector} --{k}"); }
            /* i18n-ok: css parsing and build checks */ assert_eq!(palette.len(), want.len(), "{selector}: theme.rs has tokens the css lacks");
            let t = tokens(light);
            /* i18n-ok: css parsing and build checks */ for (k, v) in &want { assert_eq!(t.iter().find(|(n, _)| n == k).unwrap().1, *v, "tokens({light}) --{k}"); }
        }
    }
}
