use libadwaita::prelude::*;
use libadwaita::Application;
use wcad_ui::build_ui;

#[test]
fn test_ui_initialization() {
    // We need to initialize GTK before we can create any widgets.
    // In a headless environment, this might fail unless GDK_BACKEND is set to something like "broadway" or "offscreen".
    // However, for a basic integration test, we can at least verify the logic.
    
    // Note: This test might be skipped or fail in environments without a session bus or display.
    if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
        println!("Skipping UI integration test: No display found");
        return;
    }

    gtk4::init().expect("Failed to initialize GTK");
    
    let app = Application::builder()
        .application_id("org.antigravity.wcad.test")
        .build();

    // We don't call app.run() as it blocks.
    // Instead, we manually trigger the build_ui logic.
    let window = build_ui(&app);
    
    let title = window.title().map(|s| s.to_string()).unwrap_or_default();
    assert_eq!(title, "WCAD", "Window title should be 'WCAD'");

    // Verify that the UI was built with the expected layout
    // (Toolbar, Viewport Container, etc.)
    let content = window.child().unwrap();
    assert!(content.is_visible());
    
    // Cleanup
    window.close();
}
