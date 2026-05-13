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

    fn check_widget(widget: &gtk4::Widget, sidebar_found: &mut bool, viewport_found: &mut bool) {
        if let Some(box_widget) = widget.downcast_ref::<gtk4::Box>() {
            // Sidebar is identified by its width request
            if box_widget.width_request() == 200 {
                *sidebar_found = true;
            }
            // Viewport container is vertical and contains DrawingArea
            else if box_widget.orientation() == gtk4::Orientation::Vertical {
                // Check if it has a DrawingArea inside
                let mut next = box_widget.first_child();
                while let Some(child) = next {
                    if child.downcast_ref::<gtk4::DrawingArea>().is_some() || child.downcast_ref::<gtk4::Frame>().is_some() {
                        *viewport_found = true;
                        break;
                    }
                    next = child.next_sibling();
                }
            }
        }
        
        // Recurse into containers if not found yet
        if !*sidebar_found || !*viewport_found {
            let mut next = widget.first_child();
            while let Some(child) = next {
                check_widget(&child, sidebar_found, viewport_found);
                next = child.next_sibling();
            }
        }
    }

    let mut next = main_box.first_child();
    while let Some(widget) = next {
        child_count += 1;
        check_widget(&widget, &mut sidebar_found, &mut viewport_found);
        next = widget.next_sibling();
    }

    assert!(child_count >= 2, "Should have at least 2 top-level UI components (Toolbar, Paned)");
    assert!(sidebar_found, "Sidebar should be present");
    assert!(viewport_found, "Viewport container should be present");
    
    window.close();
}
