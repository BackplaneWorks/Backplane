//! Viewer pages that are not the plot canvas: 3D / Mech (the hub's `solid` frames, on the GPU or else a CPU rasterizer, drawn as one image and
//! redrawn only on input) and the Browser tab (the hub's JPEG frames, polled only while the page exists). Each page is a gpui entity
//! (`Entity<SolidPage>`, `Entity<BrowserPage>`): screens::viewer hosts them with `.child(entity.clone())` next to `plot_page`.
pub mod browser;
pub mod chrome;
#[cfg(feature = "cef")]
pub mod cef;
#[cfg(feature = "cef")]
pub mod cef_page;
pub mod demo;
pub mod downloads;
pub mod layers;
pub mod raster;
pub mod solid;
#[cfg(target_os = "linux")]
pub mod solid_gpu;
