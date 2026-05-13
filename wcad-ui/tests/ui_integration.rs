use libadwaita::prelude::*;
use libadwaita::Application;
use wcad_ui::build_ui;

#[test]
fn test_ui_initialization() {
    // We need to initialize GTK before we can create any widgets.
    if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
        println!("Skipping UI integration test: No display found");
        return;
    }

    gtk4::init().expect("Failed to initialize GTK");
    
    let app = Application::builder()
        .application_id("org.antigravity.wcad.test")
        .build();

    // Directly call build_ui to inspect the resulting window
    let window = build_ui(&app);
    
    assert_eq!(window.title(), Some("WCAD".into()));
    
    // In libadwaita, we use content() to get the main widget
    let content = window.content();
    assert!(content.is_some(), "Window should have content");

    let main_box = content.unwrap().downcast::<gtk4::Box>().expect("Main content should be a Box");
    
    let mut child_count = 0;
    let mut sidebar_found = false;
    let mut viewport_found = false;

    let mut next = main_box.first_child();
    while let Some(widget) = next {
        child_count += 1;
        
        if let Some(box_widget) = widget.downcast_ref::<gtk4::Box>() {
            // Sidebar is identified by its width request
            if box_widget.width_request() == 200 {
                sidebar_found = true;
            }
            // Viewport container is vertical
            else if box_widget.orientation() == gtk4::Orientation::Vertical {
                viewport_found = true;
            }
        }

        next = widget.next_sibling();
    }

    assert!(child_count >= 3, "Should have at least 3 main UI components (Toolbar, Viewport, Sidebar)");
    assert!(sidebar_found, "Sidebar should be present");
    assert!(viewport_found, "Viewport container should be present");
    
    window.close();
}
