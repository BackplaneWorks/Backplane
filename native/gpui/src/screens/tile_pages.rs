//! The page below a dock header. Hub rows own the source and focus; ordinary
//! page actions (file rows, review tabs, diff controls) operate on the focused page.
use super::{
    filesui,
    pane_header::Tool,
    panestate::{self, PaneState},
    viewer, Screens,
};
use crate::{
    hub::Command,
    i18n::{t, trs},
    Backplane,
};
use gpui::{prelude::*, *};
use serde_json::Value;

fn focused(thread: &Value, row: &Value) -> bool {
    thread["viewer"]["layout"]["focus"] == row["id"]
}

fn picture<'a>(thread: &'a Value, row: &'a Value, kind: &str) -> &'a Value {
    let local = &row[kind];
    if focused(thread, row) {
        let front = if kind == "diff" {
            &thread["diff"]
        } else {
            &thread["viewer"][kind]
        };
        if front.is_object()
            && (kind != "files" || front["dir"] == local["dir"] || !local.is_object())
        {
            return front;
        }
    }
    local
}

// File actions resolve a row by its visible index. Use the same listing for
// input and paint even when two leaves show different directories.
pub(super) fn focused_files(thread: &Value) -> &Value {
    let viewer = &thread["viewer"];
    if let Some(id) = viewer["layout"]["focus"].as_str() {
        let row = &viewer["tiles"][id];
        if crate::screens::viewer::is_tiled(viewer) && row["kind"] == "files" {
            return picture(thread, row, "files");
        }
    }
    &viewer["files"]
}

pub(super) fn tools(
    cx: &Context<Backplane>,
    screens: &Screens,
    thread: &Value,
    row: &Value,
) -> Vec<Tool> {
    if matches!(
        row["kind"].as_str(),
        Some("board" | "gerbers" | "schematic")
    ) {
        let Some(id) = row["id"].as_str() else {
            return vec![];
        };
        let Some(frame) = screens.tile_frame(id) else {
            return vec![];
        };
        let Some(slot) = screens.plots.tile_slot(id) else {
            return vec![];
        };
        if row["error"].is_string() || row["empty"].is_string() || !frame.none.is_empty() {
            return vec![];
        }
        let present = frame
            .chunks
            .iter()
            .map(|chunk| (chunk.layer, chunk.color))
            .collect();
        let strip = crate::viewers::layers::strip(row, &present);
        if strip.rows.is_empty() {
            return vec![];
        }
        let mut layers = super::pane_header::button(
            cx,
            format!("tile:{id}:layers"),
            trs(&strip.head),
            super::pane_header::Press::TileLayers(id.to_owned()),
        );
        layers.active = (focused(thread, row) && screens.plots[slot].layers_open) || strip.mode > 0;
        return vec![layers];
    }
    if !focused(thread, row) {
        return vec![];
    }
    let kind = row["kind"].as_str().unwrap_or("");
    let data = picture(thread, row, kind);
    if !data.is_object() {
        return vec![];
    }
    match kind {
        "files" if thread["viewer"]["open"] == "files" => filesui::files_tools(cx, data),
        "diff" => viewer::diff_tools(cx, data),
        "review" => viewer::review_tools(cx, data),
        _ => vec![],
    }
}

fn missing(light: bool, focused: bool) -> AnyElement {
    let state = if focused {
        PaneState::loading(trs("Opening…"))
    } else {
        PaneState::empty(t("viewer.tile.focus"), String::new())
    };
    panestate::view(light, &state, None)
}

// Native browser surfaces belong to the thread. Mount one surface even if
// several Browser leaves are present, preferring the visible focused one.
fn browser_tile(viewer: &Value) -> Option<&str> {
    let tiles = viewer["tiles"].as_object()?;
    if let Some(id) = viewer["layout"]["maximized"].as_str() {
        return (tiles.get(id)?["kind"] == "browser").then_some(id);
    }
    if let Some(id) = viewer["layout"]["focus"].as_str() {
        if tiles.get(id).is_some_and(|t| t["kind"] == "browser") {
            return Some(id);
        }
    }
    tiles
        .iter()
        .find(|(_, t)| t["kind"] == "browser")
        .map(|(id, _)| id.as_str())
}

pub(super) fn body_with_panel(
    screens: &Screens,
    thread: &Value,
    row: &Value,
    light: bool,
    cx: &mut Context<Backplane>,
    review_panel: Option<AnyElement>,
) -> Option<AnyElement> {
    let shared_scroll = review_panel.is_some();
    let id = row["id"].as_str()?;
    let kind = row["kind"].as_str()?;
    let active = focused(thread, row);
    let v = &thread["viewer"];
    let data = picture(thread, row, kind);
    let body = match kind {
        "files"
            if active
                && matches!(v["open"].as_str(), Some("image" | "pdf" | "text"))
                && v["file"].is_object() =>
        {
            if let Some(reason) = v["file"]["error"].as_str().filter(|s| !s.is_empty()) {
                return Some(panestate::view(
                    light,
                    &PaneState::error(trs(reason), false),
                    None,
                ));
            }
            if v["file"]["loading"] == true {
                return Some(missing(light, true));
            }
            filesui::preview_page(
                cx,
                &screens.reg,
                light,
                v["open"].as_str()?,
                v,
                &screens.files,
                &screens.images,
                screens.attacher.target.as_ref(),
                Empty.into_any_element(),
            )
        }
        "files" | "diff" | "review" if data["error"].as_str().is_some_and(|s| !s.is_empty()) => {
            let retry = active
                && data["retryable"].as_bool().unwrap_or_else(|| {
                    panestate::is_retryable_none(data["error"].as_str().unwrap_or(""))
                })
                && matches!(kind, "files" | "review");
            let state = PaneState::error(trs(data["error"].as_str()?), retry);
            let button = retry.then(|| {
                let (action, value) = if kind == "files" {
                    ("fm-cd", data["dir"].as_str().unwrap_or("").to_owned())
                } else {
                    ("rv-run", String::new())
                };
                panestate::retry_btn(
                    cx,
                    light,
                    format!("tile:{id}:retry"),
                    true,
                    move |app, _| {
                        let _ = app
                            .tx
                            .try_send(Command::Action(action.into(), value.clone()));
                    },
                )
            });
            panestate::view(light, &state, button)
        }
        "files" if data["ready"] == false => {
            panestate::view(light, &PaneState::loading(trs("Loading…")), None)
        }
        "files" if !data["rows"].is_array() => missing(light, active),
        "files" => {
            let inactive = filesui::FilesUi::default();
            filesui::files_body(
                cx,
                light,
                data,
                if active { &screens.files } else { &inactive },
                None,
            )
        }
        "diff" if data["busy"] == true || data["ready"] == false => {
            panestate::view(light, &PaneState::loading(trs("Loading…")), None)
        }
        "diff" if data["files"].is_array() => viewer::diff_body(cx, light, data, None),
        "review" if data.is_object() => {
            viewer::review_body(cx, &screens.reg, light, data, None, None, review_panel)
        }
        "diff" | "review" => missing(light, active),
        "browser" if browser_tile(v) != Some(id) => missing(light, false),
        "browser" => match &screens.browser {
            Some((owner, page))
                if thread["id"].as_str().and_then(|s| s.rsplit('|').next())
                    == Some(owner.as_str()) =>
            {
                page.clone().into_any_element()
            }
            _ if screens.attacher.target.is_some() => missing(light, true),
            _ => panestate::view(
                light,
                &PaneState::error(trs("The browser page needs a connected hub."), false),
                None,
            ),
        },
        "simulation" => panestate::view(
            light,
            &PaneState::empty(t("viewer.tile.simulation.single"), String::new()),
            None,
        ),
        _ => return None,
    };
    let viewport = div()
        .id(SharedString::from(format!("tile-content-{id}")))
        .relative()
        .size_full()
        .min_h_0()
        .min_w_0();
    let viewport = if matches!(kind, "diff" | "review") && !shared_scroll {
        viewport.overflow_y_scroll()
    } else {
        viewport.overflow_hidden()
    };
    // Until the hub acknowledges focus, prevent a body click from invoking a
    // file or review action against another leaf's selection. The tile handles
    // the mouse-down that bubbles from the cover.
    Some(
        viewport
            .child(body)
            .when(!active && kind != "browser", |v| {
                v.child(div().absolute().inset_0().occlude())
            })
            .into_any_element(),
    )
}

/// A missing or busy payload has no settled state. The script also checks that
/// this leaf's body was painted before accepting the result.
pub(crate) fn settled(
    screens: &Screens,
    thread: &Value,
    row: &Value,
    cx: &App,
) -> Option<&'static str> {
    let kind = row["kind"].as_str()?;
    let data = picture(thread, row, kind);
    if row["error"].as_str().is_some_and(|e| !e.is_empty())
        || data["error"].as_str().is_some_and(|e| !e.is_empty())
    {
        return Some("error");
    }
    if row["empty"].is_string() {
        return Some("empty");
    }
    if kind == "review" && focused(thread, row) && !data["empty"].is_object() {
        if let Some((state, _)) = &screens.mgmt.review_snapshot {
            return (*state != "loading").then_some(*state);
        }
    }
    if data["busy"] == true || data["ready"] == false {
        return None;
    }
    let v = &thread["viewer"];
    match kind {
        "simulation" => Some("empty"),
        "files" => {
            let file = &v["file"];
            let image = |url: &str| match screens.images.peek(url)? {
                super::images::Slot::Ready(_) => Some("ready"),
                super::images::Slot::Failed => Some("error"),
                super::images::Slot::Loading => None,
            };
            if focused(thread, row) && file.is_object() {
                if file["error"].as_str().is_some_and(|s| !s.is_empty()) {
                    return Some("error");
                }
                if file["loading"] == true {
                    return None;
                }
                match v["open"].as_str() {
                    Some("text") if file["blocks"].as_array().is_some_and(|b| !b.is_empty()) => {
                        return Some("ready")
                    }
                    Some("text") => {
                        return match screens.files.texts.get(file["url"].as_str()?)? {
                            filesui::Text::Ready(_) => Some("ready"),
                            filesui::Text::Failed(_) => Some("error"),
                            _ => None,
                        }
                    }
                    Some("image") => return image(file["url"].as_str()?),
                    Some("pdf") if !screens.files.pdf.err.is_empty() => return Some("error"),
                    Some("pdf") if screens.files.pdf.png.is_empty() => return None,
                    Some("pdf") => {
                        return image(&format!(
                            "/img?path={}",
                            super::attach::enc(&screens.files.pdf.png)
                        ))
                    }
                    _ => {}
                }
            }
            data["rows"]
                .as_array()
                .map(|rows| if rows.is_empty() { "empty" } else { "ready" })
        }
        "diff" => data["files"]
            .as_array()
            .map(|rows| if rows.is_empty() { "empty" } else { "ready" }),
        "review" if data["empty"].is_object() => Some("empty"),
        "review" if data["checks"].is_object() || data["release"].is_object() => Some("ready"),
        "review" => {
            data["blocks"]
                .as_array()
                .map(|blocks| if blocks.is_empty() { "empty" } else { "ready" })
        }
        "browser" if browser_tile(v) == row["id"].as_str() => match &screens.browser {
            Some((owner, page))
                if thread["id"].as_str()?.rsplit('|').next() == Some(owner.as_str()) =>
            {
                page.read(cx).settled(cx)
            }
            _ if screens.attacher.target.is_none() => Some("error"),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    use serde_json::json;

    #[test]
    fn background_leaf_never_reads_focused_directory() {
        let thread = json!({"viewer":{"layout":{"focus":"b"},"files":{"dir":"b","rows":[1]}}});
        let row = json!({"id":"a","kind":"files","files":{"dir":"a"}});
        assert!(picture(&thread, &row, "files")["rows"].is_null());
    }

    #[test]
    fn file_actions_use_the_listing_painted_by_the_focused_leaf() {
        let thread = json!({"viewer":{"open":"files","splitLimits":{},"layout":{"focus":"a"},
            "files":{"dir":"stale","rows":[{"label":"wrong"}]},
            "tiles":{"a":{"id":"a","kind":"files","files":{"dir":"parts","rows":[{"label":"board"}]}}}}});
        let row = &thread["viewer"]["tiles"]["a"];
        assert_eq!(focused_files(&thread), picture(&thread, row, "files"));
        assert_eq!(focused_files(&thread)["rows"][0]["label"], "board");
    }

    #[test]
    fn a_native_browser_has_one_visible_owner() {
        let mut v = json!({"layout":{"focus":"b"},"tiles":{"a":{"kind":"browser"},"b":{"kind":"browser"},"c":{"kind":"files"}}});
        assert_eq!(browser_tile(&v), Some("b"));
        v["layout"]["maximized"] = json!("a");
        assert_eq!(browser_tile(&v), Some("a"));
        v["layout"]["maximized"] = json!("c");
        assert_eq!(browser_tile(&v), None);
    }
}
