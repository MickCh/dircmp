pub mod layout;
pub mod overlay;
pub mod panel;
pub mod statusbar;
pub mod theme;

/// High-level application phase, rendered by the status bar and overlays.
/// Lives in the UI layer so lower-level widgets never depend on `app`.
pub enum AppState {
    Idle,
    Scanning,
    /// Files are being compared in a background thread.
    Comparing { done: usize, total: usize },
    Ready,
}
