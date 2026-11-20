//! Presentation data shared by inline pane controls and their overflow menu.
pub struct PaneTool<A> {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub active: bool,
    pub icon: Option<&'static str>,
    pub action: A,
}

impl<A> PaneTool<A> {
    pub fn new(id: impl Into<String>, label: String, action: A) -> Self {
        Self { id: id.into(), label, enabled: true, active: false, icon: None, action }
    }
}
