pub mod renderer;
pub mod tessellator;

use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow, HeaderBar};
use gtk4::{Box, Orientation, DrawingArea, Button, Separator};
use std::rc::{Rc, Weak};
use std::cell::RefCell;
use renderer::Renderer;
use wcad_core::domain::{Entity, GeometryKind, Layer, Geometry};
use tessellator::tessellate_entities;
use nalgebra::Point2;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Tool {
    Select,
    Point,
    Line,
    Circle,
    Rectangle,
    Arc,
    Polyline,
}

pub struct ViewState {
    pub offset: [f32; 2],
    pub zoom: f32,
    pub cursor_pos: [f32; 2],
}

pub struct UndoState {
    pub entities: Vec<Entity>,
    pub layers: Vec<Layer>,
    pub active_layer_index: usize,
}

pub struct AppState {
    pub entities: Vec<Entity>,
    pub layers: Vec<Layer>,
    pub active_layer_index: usize,
    pub active_tool: Tool,
    pub click_buffer: Vec<Point2<f64>>,
    pub selected_indices: Vec<usize>,
    pub undo_stack: Vec<UndoState>,
    pub redo_stack: Vec<UndoState>,
    pub grid_size: f64,
    pub grid_enabled: bool,
}

impl AppState {
    pub fn push_undo(&mut self) {
        self.undo_stack.push(UndoState {
            entities: self.entities.clone(),
            layers: self.layers.clone(),
            active_layer_index: self.active_layer_index,
        });
        self.redo_stack.clear();
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(UndoState {
                entities: self.entities.clone(),
                layers: self.layers.clone(),
                active_layer_index: self.active_layer_index,
            });
            self.entities = prev.entities;
            self.layers = prev.layers;
            self.active_layer_index = prev.active_layer_index;
            self.selected_indices.clear();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(UndoState {
                entities: self.entities.clone(),
                layers: self.layers.clone(),
                active_layer_index: self.active_layer_index,
            });
            self.entities = next.entities;
            self.layers = next.layers;
            self.active_layer_index = next.active_layer_index;
            self.selected_indices.clear();
        }
    }

    pub fn delete_selected(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }
        self.push_undo();
        // Sort indices descending to avoid shifting issues during removal
        let mut indices = self.selected_indices.clone();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for i in indices {
            if i < self.entities.len() {
                self.entities.remove(i);
            }
        }
        self.selected_indices.clear();
    }

    pub fn snap_point(&self, point: Point2<f64>, zoom: f32) -> Point2<f64> {
        let mut snapped = point;
        let mut best_dist = 0.02 / zoom as f64; // Snap threshold

        // Snap to endpoints
        for entity in &self.entities {
            match &entity.geometry {
                GeometryKind::Line { start, end } => {
                    let d1 = (start - point).norm();
                    if d1 < best_dist {
                        best_dist = d1;
                        snapped = *start;
                    }
                    let d2 = (end - point).norm();
                    if d2 < best_dist {
                        best_dist = d2;
                        snapped = *end;
                    }
                }
                GeometryKind::Circle { center, .. } => {
                    let d = (center - point).norm();
                    if d < best_dist {
                        best_dist = d;
                        snapped = *center;
                    }
                }
                GeometryKind::Rectangle { start, end } => {
                    let corners = [
                        *start,
                        Point2::new(end.x, start.y),
                        *end,
                        Point2::new(start.x, end.y),
                    ];
                    for c in corners {
                        let d = (c - point).norm();
                        if d < best_dist {
                            best_dist = d;
                            snapped = c;
                        }
                    }
                }
                GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                    // Snap to center
                    let d_c = (center - point).norm();
                    if d_c < best_dist {
                        best_dist = d_c;
                        snapped = *center;
                    }
                    // Snap to endpoints
                    let p1 = center + nalgebra::Vector2::new(start_angle.cos() * radius, start_angle.sin() * radius);
                    let p2 = center + nalgebra::Vector2::new((start_angle + sweep_angle).cos() * radius, (start_angle + sweep_angle).sin() * radius);
                    let d1 = (p1 - point).norm();
                    if d1 < best_dist {
                        best_dist = d1;
                        snapped = p1;
                    }
                    let d2 = (p2 - point).norm();
                    if d2 < best_dist {
                        best_dist = d2;
                        snapped = p2;
                    }
                }
                GeometryKind::Polyline(points) => {
                    for p in points {
                        let d = (p - point).norm();
                        if d < best_dist {
                            best_dist = d;
                            snapped = *p;
                        }
                    }
                }
                _ => {}
            }
        }

        // Snap to grid if enabled and no endpoint found
        if self.grid_enabled && best_dist >= 0.02 / zoom as f64 {
            let x = (point.x / self.grid_size).round() * self.grid_size;
            let y = (point.y / self.grid_size).round() * self.grid_size;
            snapped = Point2::new(x, y);
        }

        snapped
    }
}

pub fn build_ui(app: &Application) -> ApplicationWindow {
    let renderer = Rc::new(RefCell::new(pollster::block_on(Renderer::new())));
    let view_state = Rc::new(RefCell::new(ViewState {
        offset: [0.0, 0.0],
        zoom: 1.0,
        cursor_pos: [0.0, 0.0],
    }));
    let app_state = Rc::new(RefCell::new(AppState {
        entities: Vec::new(),
        layers: vec![Layer::new("0", [1.0, 1.0, 1.0])],
        active_layer_index: 0,
        active_tool: Tool::Select,
        click_buffer: Vec::new(),
        selected_indices: Vec::new(),
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
        grid_size: 0.1,
        grid_enabled: true,
    }));

    let main_layout = Box::builder()
        .orientation(Orientation::Horizontal)
        .build();

    // Toolbar
    let toolbar = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_start(6)
        .margin_end(6)
        .margin_top(6)
        .margin_bottom(6)
        .build();

    let btn_select = Button::with_label("Sel");
    let btn_point = Button::with_label("Pnt");
    let btn_line = Button::with_label("Line");
    let btn_circle = Button::with_label("Circ");
    let btn_rect = Button::with_label("Rect");
    let btn_arc = Button::with_label("Arc");
    let btn_poly = Button::with_label("Poly");
    let btn_grid = gtk4::ToggleButton::with_label("Grid");
    btn_grid.set_active(true);

    let btn_open = Button::with_label("Open");
    let btn_save = Button::with_label("Save");

    toolbar.append(&btn_select);
    toolbar.append(&Separator::new(Orientation::Horizontal));
    toolbar.append(&btn_point);
    toolbar.append(&btn_line);
    toolbar.append(&btn_circle);
    toolbar.append(&btn_rect);
    toolbar.append(&btn_arc);
    toolbar.append(&btn_poly);
    toolbar.append(&Separator::new(Orientation::Horizontal));
    toolbar.append(&btn_grid);
    toolbar.append(&Separator::new(Orientation::Horizontal));
    toolbar.append(&btn_open);
    toolbar.append(&btn_save);

    main_layout.append(&toolbar);

    let viewport_container = Box::builder()
        .orientation(Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .build();

    let header = HeaderBar::builder()
        .title_widget(&libadwaita::WindowTitle::new("WCAD", "2D Drafting for Linux"))
        .build();

    viewport_container.append(&header);

    let viewport = DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .can_focus(true)
        .focusable(true)
        .build();
    let viewport_grid_ref = Rc::new(RefCell::new(Some(viewport.clone())));

    viewport_container.append(&viewport);

    // Status Bar
    let status_bar = gtk4::Label::builder()
        .label("X: 0.000, Y: 0.000")
        .halign(gtk4::Align::Start)
        .margin_start(6)
        .margin_end(6)
        .margin_top(3)
        .margin_bottom(3)
        .build();
    viewport_container.append(&status_bar);

    main_layout.append(&viewport_container);

    // Sidebar (Property Editor)
    let sidebar = Box::builder()
        .orientation(Orientation::Vertical)
        .width_request(200)
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(12)
        .build();

    let sidebar_title = gtk4::Label::builder()
        .label("Properties")
        .css_classes(["title-2"])
        .halign(gtk4::Align::Start)
        .build();
    sidebar.append(&sidebar_title);

    let props_container = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .build();
    sidebar.append(&props_container);

    let layers_container = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    sidebar.append(&Separator::new(Orientation::Horizontal));
    sidebar.append(&gtk4::Label::new(Some("Layers")));
    sidebar.append(&layers_container);
    
    let btn_add_layer = Button::with_label("Add Layer");
    sidebar.append(&btn_add_layer);

    main_layout.append(&sidebar);

    let update_sidebar_weak: Rc<RefCell<Option<Weak<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let update_sidebar: Rc<dyn Fn()> = Rc::new({
        let app_state = app_state.clone();
        let props_container = props_container.clone();
        let layers_container = layers_container.clone();
        let viewport = viewport.clone();
        let update_sidebar_weak = update_sidebar_weak.clone();
        move || {
            while let Some(child) = props_container.first_child() {
                props_container.remove(&child);
            }
            while let Some(child) = layers_container.first_child() {
                layers_container.remove(&child);
            }

            let app_val = app_state.borrow();
            
            // Build Layers List
            for (idx, layer) in app_val.layers.iter().enumerate() {
                let row = Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(6)
                    .build();
                
                let name_label = gtk4::Label::builder()
                    .label(&layer.name)
                    .hexpand(true)
                    .halign(gtk4::Align::Start)
                    .build();
                
                let is_active = idx == app_val.active_layer_index;
                let active_indicator = gtk4::Image::from_icon_name(if is_active { "emblem-ok-symbolic" } else { "non-existent" });
                
                let btn_activate = Button::builder()
                    .child(&active_indicator)
                    .tooltip_text("Make Active")
                    .build();
                
                {
                    let app_state = app_state.clone();
                    let viewport = viewport.clone();
                    let weak_cell = update_sidebar_weak.clone();
                    btn_activate.connect_clicked(move |_| {
                        {
                            app_state.borrow_mut().active_layer_index = idx;
                        }
                        viewport.queue_draw();
                        if let Some(weak) = weak_cell.borrow().as_ref() {
                            if let Some(update) = weak.upgrade() { update(); }
                        }
                    });
                }

                let btn_visible = Button::builder()
                    .icon_name(if layer.visible { "view-visible-symbolic" } else { "view-conceal-symbolic" })
                    .build();
                {
                    let app_state = app_state.clone();
                    let viewport = viewport.clone();
                    let weak_cell = update_sidebar_weak.clone();
                    btn_visible.connect_clicked(move |_| {
                        {
                            let mut app = app_state.borrow_mut();
                            app.push_undo();
                            app.layers[idx].visible = !app.layers[idx].visible;
                        }
                        viewport.queue_draw();
                        if let Some(weak) = weak_cell.borrow().as_ref() {
                            if let Some(update) = weak.upgrade() { update(); }
                        }
                    });
                }

                row.append(&name_label);
                row.append(&btn_activate);
                row.append(&btn_visible);
                layers_container.append(&row);
            }

            let app = app_val;
            if app.selected_indices.len() == 1 {
                let index = app.selected_indices[0];
                let entity = app.entities[index].clone();
                
                match &entity.geometry {
                    GeometryKind::Point(p) => {
                        props_container.append(&gtk4::Label::new(Some("Type: Point")));
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "X", p.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Point(ref mut p) = app.entities[index].geometry { p.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Y", p.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Point(ref mut p) = app.entities[index].geometry { p.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                    }
                    GeometryKind::Line { start, end } => {
                        props_container.append(&gtk4::Label::new(Some("Type: Line")));
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Start X", start.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Line { ref mut start, .. } = app.entities[index].geometry { start.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Start Y", start.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Line { ref mut start, .. } = app.entities[index].geometry { start.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "End X", end.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Line { ref mut end, .. } = app.entities[index].geometry { end.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "End Y", end.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Line { ref mut end, .. } = app.entities[index].geometry { end.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                    }
                    GeometryKind::Circle { center, radius } => {
                        props_container.append(&gtk4::Label::new(Some("Type: Circle")));
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Center X", center.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Circle { ref mut center, .. } = app.entities[index].geometry { center.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Center Y", center.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Circle { ref mut center, .. } = app.entities[index].geometry { center.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Radius", *radius, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Circle { ref mut radius, .. } = app.entities[index].geometry { *radius = val.max(0.0); }
                                }
                                viewport.queue_draw();
                            });
                        }
                    }
                    GeometryKind::Rectangle { start, end } => {
                        props_container.append(&gtk4::Label::new(Some("Type: Rectangle")));
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Start X", start.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Rectangle { ref mut start, .. } = app.entities[index].geometry { start.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Start Y", start.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Rectangle { ref mut start, .. } = app.entities[index].geometry { start.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "End X", end.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Rectangle { ref mut end, .. } = app.entities[index].geometry { end.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "End Y", end.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Rectangle { ref mut end, .. } = app.entities[index].geometry { end.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                    }
                    GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                        props_container.append(&gtk4::Label::new(Some("Type: Arc")));
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Center X", center.x, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Arc { ref mut center, .. } = app.entities[index].geometry { center.x = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Center Y", center.y, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Arc { ref mut center, .. } = app.entities[index].geometry { center.y = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Radius", *radius, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Arc { ref mut radius, .. } = app.entities[index].geometry { *radius = val.max(0.0); }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Start Angle", *start_angle, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Arc { ref mut start_angle, .. } = app.entities[index].geometry { *start_angle = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                        {
                            let app_state = app_state.clone();
                            let viewport = viewport.clone();
                            append_f64_prop(&props_container, "Sweep Angle", *sweep_angle, move |val| {
                                {
                                    let mut app = app_state.borrow_mut();
                                    app.push_undo();
                                    if let GeometryKind::Arc { ref mut sweep_angle, .. } = app.entities[index].geometry { *sweep_angle = val; }
                                }
                                viewport.queue_draw();
                            });
                        }
                    }
                    GeometryKind::Polyline(ref points) => {
                        props_container.append(&gtk4::Label::new(Some(&format!("Type: Polyline ({} pts)", points.len()))));
                    }
                }
            } else if app.selected_indices.is_empty() {
                props_container.append(&gtk4::Label::new(Some("No selection")));
            } else {
                props_container.append(&gtk4::Label::new(Some("Multiple selected")));
            }
        }
    });

    *update_sidebar_weak.borrow_mut() = Some(Rc::downgrade(&update_sidebar));
    update_sidebar();

    let app_state_point = app_state.clone();
    let update_sidebar_point = update_sidebar.clone();
    btn_point.connect_clicked(move |_| {
        {
            let mut state = app_state_point.borrow_mut();
            state.active_tool = Tool::Point;
            state.click_buffer.clear();
            state.selected_indices.clear();
        }
        update_sidebar_point();
    });

    let app_state_select = app_state.clone();
    let update_sidebar_select = update_sidebar.clone();
    btn_select.connect_clicked(move |_| {
        {
            let mut state = app_state_select.borrow_mut();
            state.active_tool = Tool::Select;
            state.click_buffer.clear();
        }
        update_sidebar_select();
    });

    let app_state_line = app_state.clone();
    let update_sidebar_line = update_sidebar.clone();
    btn_line.connect_clicked(move |_| {
        {
            let mut state = app_state_line.borrow_mut();
            state.active_tool = Tool::Line;
            state.click_buffer.clear();
            state.selected_indices.clear();
        }
        update_sidebar_line();
    });

    let app_state_circle = app_state.clone();
    let update_sidebar_circle = update_sidebar.clone();
    btn_circle.connect_clicked(move |_| {
        {
            let mut state = app_state_circle.borrow_mut();
            state.active_tool = Tool::Circle;
            state.click_buffer.clear();
            state.selected_indices.clear();
        }
        update_sidebar_circle();
    });

    let app_state_rect = app_state.clone();
    let update_sidebar_rect = update_sidebar.clone();
    btn_rect.connect_clicked(move |_| {
        {
            let mut state = app_state_rect.borrow_mut();
            state.active_tool = Tool::Rectangle;
            state.click_buffer.clear();
            state.selected_indices.clear();
        }
        update_sidebar_rect();
    });

    let app_state_arc = app_state.clone();
    let update_sidebar_arc = update_sidebar.clone();
    btn_arc.connect_clicked(move |_| {
        {
            let mut state = app_state_arc.borrow_mut();
            state.active_tool = Tool::Arc;
            state.click_buffer.clear();
            state.selected_indices.clear();
        }
        update_sidebar_arc();
    });

    let app_state_poly = app_state.clone();
    let update_sidebar_poly = update_sidebar.clone();
    btn_poly.connect_clicked(move |_| {
        {
            let mut state = app_state_poly.borrow_mut();
            state.active_tool = Tool::Polyline;
            state.click_buffer.clear();
            state.selected_indices.clear();
        }
        update_sidebar_poly();
    });

    let app_state_grid = app_state.clone();
    let viewport_grid_ref_closure = viewport_grid_ref.clone();
    btn_grid.connect_toggled(move |btn| {
        let mut state = app_state_grid.borrow_mut();
        state.grid_enabled = btn.is_active();
        if let Some(vp) = viewport_grid_ref_closure.borrow().as_ref() {
            let vp: &DrawingArea = vp;
            vp.queue_draw();
        }
    });

    let app_state_open = app_state.clone();
    let viewport_open_ref = viewport_grid_ref.clone();
    let update_sidebar_open = update_sidebar.clone();
    btn_open.connect_clicked(move |_| {
        let state = app_state_open.clone();
        let viewport_ref = viewport_open_ref.clone();
        let update_sidebar = update_sidebar_open.clone();
        let dialog = gtk4::FileDialog::builder()
            .title("Open WCAD File")
            .build();
        
        dialog.open(None::<&ApplicationWindow>, None::<&gtk4::gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if let Ok(entities) = serde_json::from_str::<Vec<Entity>>(&content) {
                            {
                                let mut app = state.borrow_mut();
                                app.push_undo();
                                app.entities = entities;
                                app.selected_indices.clear();
                            }
                            update_sidebar();
                            if let Some(vp) = viewport_ref.borrow().as_ref() {
                                vp.queue_draw();
                            }
                        }
                    }
                }
            }
        });
    });

    let app_state_save = app_state.clone();
    btn_save.connect_clicked(move |_| {
        let state = app_state_save.clone();
        let dialog = gtk4::FileDialog::builder()
            .title("Save WCAD File")
            .initial_name("drawing.json")
            .build();
        
        dialog.save(None::<&ApplicationWindow>, None::<&gtk4::gio::Cancellable>, move |res| {
            if let Ok(file) = res {
                if let Some(path) = file.path() {
                    let entities = state.borrow().entities.clone();
                    if let Ok(content) = serde_json::to_string_pretty(&entities) {
                        let _ = std::fs::write(path, content);
                    }
                }
            }
        });
    });

    // Keyboard Shortcuts
    let key_controller = gtk4::EventControllerKey::new();
    let app_state_key = app_state.clone();
    let viewport_key = viewport.clone();
    let update_sidebar_key = update_sidebar.clone();
    key_controller.connect_key_pressed(move |_controller, key, _code, state| {
        let mut app = app_state_key.borrow_mut();
        let mut handled = false;

        match key {
            gtk4::gdk::Key::Delete => {
                app.delete_selected();
                handled = true;
            }
            gtk4::gdk::Key::z if state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) => {
                app.undo();
                handled = true;
            }
            gtk4::gdk::Key::y if state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) => {
                app.redo();
                handled = true;
            }
            gtk4::gdk::Key::g => {
                app.grid_enabled = !app.grid_enabled;
                handled = true;
            }
            gtk4::gdk::Key::Escape => {
                app.click_buffer.clear();
                handled = true;
            }
            gtk4::gdk::Key::Return | gtk4::gdk::Key::KP_Enter => {
                if app.active_tool == Tool::Polyline && app.click_buffer.len() >= 2 {
                    app.push_undo();
                    let layer_name = app.layers[app.active_layer_index].name.clone();
                    let poly = Entity::new(GeometryKind::Polyline(app.click_buffer.clone()), &layer_name);
                    app.entities.push(poly);
                    app.click_buffer.clear();
                    handled = true;
                }
            }
            _ => {}
        }

        if handled {
            { drop(app); } // Explicitly drop mutable borrow
            update_sidebar_key();
            viewport_key.queue_draw();
            gtk4::glib::Propagation::Stop
        } else {
            gtk4::glib::Propagation::Proceed
        }
    });
    viewport.add_controller(key_controller);

    let app_state_add_layer = app_state.clone();
    let viewport_add_layer = viewport.clone();
    let update_sidebar_add_layer = update_sidebar.clone();
    btn_add_layer.connect_clicked(move |_| {
        {
            let mut app = app_state_add_layer.borrow_mut();
            app.push_undo();
            let name = format!("Layer {}", app.layers.len());
            app.layers.push(Layer::new(&name, [1.0, 1.0, 1.0]));
        }
        viewport_add_layer.queue_draw();
        update_sidebar_add_layer();
    });

    // Motion tracking
    let motion_controller = gtk4::EventControllerMotion::new();
    let view_state_motion = view_state.clone();
    let app_state_motion = app_state.clone();
    let viewport_motion = viewport.clone();
    let status_bar_motion = status_bar.clone();
    motion_controller.connect_motion(move |_controller, x, y| {
        let mut view = view_state_motion.borrow_mut();
        let app = app_state_motion.borrow();
        view.cursor_pos = [x as f32, y as f32];
        
        let world = pixel_to_world(
            x as f32, y as f32, 
            viewport_motion.width() as f32, viewport_motion.height() as f32, 
            view.offset, view.zoom
        );
        let world_point = Point2::from(world);
        let snapped = app.snap_point(world_point, view.zoom);
        
        status_bar_motion.set_label(&format!("X: {:.3}, Y: {:.3}", snapped.x, snapped.y));
        
        viewport_motion.queue_draw();
    });
    viewport.add_controller(motion_controller);

    // Left Click Interaction (Tool Usage & Selection)
    let click_gesture = gtk4::GestureClick::new();
    let app_state_click = app_state.clone();
    let view_state_click = view_state.clone();
    let viewport_click = viewport.clone();
    let update_sidebar_click = update_sidebar.clone();
    click_gesture.connect_pressed(move |_gesture, _n_press, x, y| {
        viewport_click.grab_focus();
        {
            let mut app = app_state_click.borrow_mut();
            let view = view_state_click.borrow();
            
            let world_pos = pixel_to_world(
                x as f32, y as f32, 
                viewport_click.width() as f32, viewport_click.height() as f32, 
                view.offset, view.zoom
            );

            let world_point = Point2::from(world_pos);
            let snapped = app.snap_point(world_point, view.zoom);

            match app.active_tool {
                Tool::Point => {
                    app.push_undo();
                    let layer_name = app.layers[app.active_layer_index].name.clone();
                    app.entities.push(Entity::new(GeometryKind::Point(snapped), &layer_name));
                }
                Tool::Line => {
                    if app.click_buffer.is_empty() {
                        app.click_buffer.push(snapped);
                    } else {
                        app.push_undo();
                        let start = app.click_buffer[0];
                        let layer_name = app.layers[app.active_layer_index].name.clone();
                        app.entities.push(Entity::new(GeometryKind::Line { start, end: snapped }, &layer_name));
                        app.click_buffer.clear();
                    }
                }
                Tool::Circle => {
                    if app.click_buffer.is_empty() {
                        app.click_buffer.push(snapped);
                    } else {
                        app.push_undo();
                        let center = app.click_buffer[0];
                        let radius = ((center.x - snapped.x).powi(2) + (center.y - snapped.y).powi(2)).sqrt();
                        let layer_name = app.layers[app.active_layer_index].name.clone();
                        app.entities.push(Entity::new(GeometryKind::Circle { center, radius }, &layer_name));
                        app.click_buffer.clear();
                    }
                }
                Tool::Rectangle => {
                    if app.click_buffer.is_empty() {
                        app.click_buffer.push(snapped);
                    } else {
                        app.push_undo();
                        let start = app.click_buffer[0];
                        let layer_name = app.layers[app.active_layer_index].name.clone();
                        app.entities.push(Entity::new(GeometryKind::Rectangle { start, end: snapped }, &layer_name));
                        app.click_buffer.clear();
                    }
                }
                Tool::Arc => {
                    if app.click_buffer.len() < 2 {
                        app.click_buffer.push(snapped);
                    } else {
                        app.push_undo();
                        let center = app.click_buffer[0];
                        let p1 = app.click_buffer[1];
                        let radius = ((center.x - p1.x).powi(2) + (center.y - p1.y).powi(2)).sqrt();
                        let start_angle = (p1.y - center.y).atan2(p1.x - center.x);
                        let end_angle = (snapped.y - center.y).atan2(snapped.x - center.x);
                        let mut sweep_angle = end_angle - start_angle;
                        if sweep_angle < 0.0 { sweep_angle += 2.0 * std::f64::consts::PI; }
                        let layer_name = app.layers[app.active_layer_index].name.clone();
                        app.entities.push(Entity::new(GeometryKind::Arc { center, radius, start_angle, sweep_angle }, &layer_name));
                        app.click_buffer.clear();
                    }
                }
                Tool::Polyline => {
                    app.click_buffer.push(snapped);
                }
                Tool::Select => {
                    let mut closest = None;
                    let mut min_dist = 0.02 / view.zoom as f64;
                    
                    for (i, entity) in app.entities.iter().enumerate() {
                        let dist = entity.distance_to(&world_point);
                        if dist < min_dist {
                            min_dist = dist;
                            closest = Some(i);
                        }
                    }
                    
                    app.selected_indices.clear();
                    if let Some(index) = closest {
                        app.selected_indices.push(index);
                    }
                }
            }
        }
        update_sidebar_click();
        viewport_click.queue_draw();
    });
    viewport.add_controller(click_gesture);
    
    // Right Click Interaction (Finish/Cancel)
    let right_click = gtk4::GestureClick::new();
    right_click.set_button(3);
    let app_state_rc = app_state.clone();
    let viewport_rc = viewport.clone();
    right_click.connect_pressed(move |_gesture, _n, _x, _y| {
        let mut app = app_state_rc.borrow_mut();
        if app.active_tool == Tool::Polyline && app.click_buffer.len() >= 2 {
            app.push_undo();
            let layer_name = app.layers[app.active_layer_index].name.clone();
            let poly = Entity::new(GeometryKind::Polyline(app.click_buffer.clone()), &layer_name);
            app.entities.push(poly);
            app.click_buffer.clear();
        } else {
            app.click_buffer.clear();
        }
        viewport_rc.queue_draw();
    });
    viewport.add_controller(right_click);

    // Zoom handling (Scroll)
    let scroll_controller = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    let view_state_scroll = view_state.clone();
    let viewport_scroll = viewport.clone();
    scroll_controller.connect_scroll(move |_controller, _dx, dy| {
        let mut state = view_state_scroll.borrow_mut();
        let old_zoom = state.zoom;
        let zoom_factor = 1.1f32;
        
        if dy < 0.0 {
            state.zoom *= zoom_factor;
        } else {
            state.zoom /= zoom_factor;
        }

        let new_zoom = state.zoom;
        let w = viewport_scroll.width() as f32;
        let h = viewport_scroll.height() as f32;
        
        let world_cursor = pixel_to_world(state.cursor_pos[0], state.cursor_pos[1], w, h, state.offset, old_zoom);

        state.offset[0] = world_cursor[0] as f32 - (world_cursor[0] as f32 - state.offset[0]) * (old_zoom / new_zoom);
        state.offset[1] = world_cursor[1] as f32 - (world_cursor[1] as f32 - state.offset[1]) * (old_zoom / new_zoom);

        viewport_scroll.queue_draw();
        gtk4::glib::Propagation::Proceed
    });
    viewport.add_controller(scroll_controller);

    // Pan handling
    let drag_gesture = gtk4::GestureDrag::new();
    drag_gesture.set_button(2); 
    let view_state_drag = view_state.clone();
    let viewport_drag = viewport.clone();
    let start_offset = Rc::new(RefCell::new([0.0f32; 2]));

    let start_offset_begin = start_offset.clone();
    let view_state_begin = view_state_drag.clone();
    drag_gesture.connect_drag_begin(move |_gesture, _x, _y| {
        *start_offset_begin.borrow_mut() = view_state_begin.borrow().offset;
    });

    let start_offset_update = start_offset.clone();
    let view_state_update = view_state_drag.clone();
    let viewport_update = viewport_drag.clone();
    drag_gesture.connect_drag_update(move |_gesture, offset_x, offset_y| {
        let mut state = view_state_update.borrow_mut();
        let start = start_offset_update.borrow();
        let scale = 2.0 / (viewport_update.height() as f32 * state.zoom);
        state.offset[0] = start[0] - (offset_x as f32 * scale);
        state.offset[1] = start[1] + (offset_y as f32 * scale);
        viewport_update.queue_draw();
    });
    viewport.add_controller(drag_gesture);

    // Drawing
    let renderer_draw = renderer.clone();
    let view_state_draw = view_state.clone();
    let app_state_draw = app_state.clone();
    viewport.set_draw_func(move |_area, cr, width, height| {
        let view = view_state_draw.borrow();
        let app = app_state_draw.borrow();
        let mut renderer = renderer_draw.borrow_mut();
        
        renderer.update_view(view.offset, view.zoom, width as f32, height as f32);

        let mut render_entities: Vec<(Entity, [f32; 3])> = Vec::new();

        // Background Grid
        if app.grid_enabled {
            let grid_size = app.grid_size;
            let w = width as f32;
            let h = height as f32;
            let min_world = pixel_to_world(0.0, h, w, h, view.offset, view.zoom);
            let max_world = pixel_to_world(w, 0.0, w, h, view.offset, view.zoom);
            
            let start_x = (min_world[0] as f64 / grid_size).floor() * grid_size;
            let end_x = (max_world[0] as f64 / grid_size).ceil() * grid_size;
            let start_y = (min_world[1] as f64 / grid_size).floor() * grid_size;
            let end_y = (max_world[1] as f64 / grid_size).ceil() * grid_size;
            
            let x_steps = ((end_x - start_x) / grid_size).round() as i32;
            let y_steps = ((end_y - start_y) / grid_size).round() as i32;
            
            if x_steps < 100 && y_steps < 100 {
                for i in 0..=x_steps {
                    let x = start_x + i as f64 * grid_size;
                    for j in 0..=y_steps {
                        let y = start_y + j as f64 * grid_size;
                        render_entities.push((Entity::new(GeometryKind::Point(Point2::new(x, y)), "grid"), [0.2, 0.2, 0.2]));
                    }
                }
            }
        }

        // Document Entities
        for (i, e) in app.entities.iter().enumerate() {
            if let Some(layer) = app.layers.iter().find(|l| l.name == e.layer) {
                if !layer.visible { continue; }
            }
            let color = if app.selected_indices.contains(&i) {
                [1.0, 1.0, 0.0]
            } else {
                app.layers.iter().find(|l| l.name == e.layer).map(|l| l.color).unwrap_or([1.0, 1.0, 1.0])
            };
            render_entities.push((e.clone(), color));
        }

        // Rubber-banding & Snap Preview
        let mouse_world = pixel_to_world(view.cursor_pos[0], view.cursor_pos[1], width as f32, height as f32, view.offset, view.zoom);
        let mouse_point = Point2::from(mouse_world);
        let snapped = app.snap_point(mouse_point, view.zoom);

        if app.active_tool != Tool::Select {
            render_entities.push((Entity::new(GeometryKind::Circle { center: snapped, radius: 0.005 / view.zoom as f64 }, "preview"), [0.0, 1.0, 0.0]));
        }

        if !app.click_buffer.is_empty() {
            match app.active_tool {
                Tool::Line => {
                    render_entities.push((Entity::new(GeometryKind::Line { start: app.click_buffer[0], end: snapped }, "preview"), [0.5, 0.5, 1.0]));
                }
                Tool::Circle => {
                    let center = app.click_buffer[0];
                    let radius = ((center.x - snapped.x).powi(2) + (center.y - snapped.y).powi(2)).sqrt();
                    render_entities.push((Entity::new(GeometryKind::Circle { center, radius }, "preview"), [0.5, 0.5, 1.0]));
                }
                Tool::Rectangle => {
                    render_entities.push((Entity::new(GeometryKind::Rectangle { start: app.click_buffer[0], end: snapped }, "preview"), [0.5, 0.5, 1.0]));
                }
                Tool::Arc => {
                    let center = app.click_buffer[0];
                    if app.click_buffer.len() == 1 {
                        render_entities.push((Entity::new(GeometryKind::Line { start: center, end: snapped }, "preview"), [0.5, 0.5, 1.0]));
                    } else if app.click_buffer.len() == 2 {
                        let p1 = app.click_buffer[1];
                        let radius = ((center.x - p1.x).powi(2) + (center.y - p1.y).powi(2)).sqrt();
                        let start_angle = (p1.y - center.y).atan2(p1.x - center.x);
                        let end_angle = (snapped.y - center.y).atan2(snapped.x - center.x);
                        let mut sweep_angle = end_angle - start_angle;
                        if sweep_angle < 0.0 { sweep_angle += 2.0 * std::f64::consts::PI; }
                        render_entities.push((Entity::new(GeometryKind::Arc { center, radius, start_angle, sweep_angle }, "preview"), [0.5, 0.5, 1.0]));
                    }
                }
                Tool::Polyline => {
                    let mut pts = app.click_buffer.clone();
                    pts.push(snapped);
                    render_entities.push((Entity::new(GeometryKind::Polyline(pts), "preview"), [0.5, 0.5, 1.0]));
                }
                _ => {}
            }
        }

        let refs: Vec<(&Entity, [f32; 3])> = render_entities.iter().map(|(e, c)| (e, *c)).collect();
        let (vertices, indices) = tessellate_entities(&refs);
        let data = renderer.render(width as u32, height as u32, &vertices, &indices);
        
        if data.is_empty() { return; }

        let mut surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, width, height).unwrap();
        {
            let mut surface_data = surface.data().unwrap();
            surface_data.copy_from_slice(&data);
        }
        cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
        cr.paint().unwrap();
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1200)
        .default_height(800)
        .title("WCAD")
        .content(&main_layout)
        .build();

    window.present();
    window
}

fn pixel_to_world(x: f32, y: f32, width: f32, height: f32, offset: [f32; 2], zoom: f32) -> [f64; 2] {
    let aspect = width / height;
    let wx = (x - width / 2.0) * (aspect / (width / 2.0)) / zoom + offset[0];
    let wy = -(y - height / 2.0) * (1.0 / (height / 2.0)) / zoom + offset[1];
    [wx as f64, wy as f64]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_to_world_center() {
        // Center of a 100x100 screen should be (0,0) world at offset 0
        let world = pixel_to_world(50.0, 50.0, 100.0, 100.0, [0.0, 0.0], 1.0);
        assert!((world[0] - 0.0).abs() < 1e-6);
        assert!((world[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_pixel_to_world_offset() {
        // Center of screen with offset [10, 10] should be (10, 10) world
        let world = pixel_to_world(50.0, 50.0, 100.0, 100.0, [10.0, 10.0], 1.0);
        assert!((world[0] - 10.0).abs() < 1e-6);
        assert!((world[1] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_pixel_to_world_zoom() {
        // At zoom 2.0, clicking 25px right of center (in 100x100) should be 0.25 units in world
        // (Since total width at zoom 1.0 is 2.0 units for 1:1 aspect)
        let world = pixel_to_world(75.0, 50.0, 100.0, 100.0, [0.0, 0.0], 2.0);
        assert!((world[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_app_state_undo_redo() {
        let mut state = AppState {
            entities: Vec::new(),
            layers: vec![Layer::new("0", [1.0, 1.0, 1.0])],
            active_layer_index: 0,
            active_tool: Tool::Select,
            click_buffer: Vec::new(),
            selected_indices: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            grid_size: 0.1,
            grid_enabled: true,
        };

        // Add an entity
        state.push_undo();
        state.entities.push(Entity::new(GeometryKind::Point(Point2::new(0.0, 0.0)), "0"));
        assert_eq!(state.entities.len(), 1);

        // Undo
        state.undo();
        assert_eq!(state.entities.len(), 0);
        assert_eq!(state.redo_stack.len(), 1);

        // Redo
        state.redo();
        assert_eq!(state.entities.len(), 1);
        assert_eq!(state.undo_stack.len(), 1);
    }

    #[test]
    fn test_app_state_delete_selected() {
        let mut state = AppState {
            entities: vec![
                Entity::new(GeometryKind::Point(Point2::new(0.0, 0.0)), "0"),
                Entity::new(GeometryKind::Point(Point2::new(1.0, 1.0)), "0"),
                Entity::new(GeometryKind::Point(Point2::new(2.0, 2.0)), "0"),
            ],
            layers: vec![Layer::new("0", [1.0, 1.0, 1.0])],
            active_layer_index: 0,
            active_tool: Tool::Select,
            click_buffer: Vec::new(),
            selected_indices: vec![0, 2], // Select first and third
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            grid_size: 0.1,
            grid_enabled: true,
        };

        state.delete_selected();
        assert_eq!(state.entities.len(), 1);
        // The one at index 1 (Point(1,1)) should remain
        if let GeometryKind::Point(p) = &state.entities[0].geometry {
            assert_eq!(p.x, 1.0);
        } else {
            panic!("Wrong entity remains");
        }

        // Undo delete
        state.undo();
        assert_eq!(state.entities.len(), 3);
    }

    #[test]
    fn test_app_state_snap_point() {
        let mut state = AppState {
            entities: vec![
                Entity::new(GeometryKind::Line {
                    start: Point2::new(0.0, 0.0),
                    end: Point2::new(1.0, 0.0),
                }, "0")
            ],
            layers: vec![Layer::new("0", [1.0, 1.0, 1.0])],
            active_layer_index: 0,
            active_tool: Tool::Select,
            click_buffer: Vec::new(),
            selected_indices: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            grid_size: 0.1,
            grid_enabled: true,
        };

        // Snap to endpoint (0,0) - within 0.02 threshold
        let snapped = state.snap_point(Point2::new(0.01, 0.01), 1.0);
        assert_eq!(snapped, Point2::new(0.0, 0.0));

        // Snap to grid (0.1, 0.1) - far from endpoints but grid enabled
        let snapped = state.snap_point(Point2::new(0.12, 0.08), 1.0);
        assert!((snapped.x - 0.1).abs() < 1e-6);
        assert!((snapped.y - 0.1).abs() < 1e-6);

        // Snap to endpoint takes priority over grid
        // Endpoint at (0,0), Grid at (0.01, 0.01) - not a grid point but let's say (0,0) is also a grid point
        // If we click at (0.01, 0.0), it should snap to endpoint (0,0)
        let snapped = state.snap_point(Point2::new(0.01, 0.0), 1.0);
        assert_eq!(snapped, Point2::new(0.0, 0.0));

        // Grid snap disabled
        state.grid_enabled = false;
        let snapped = state.snap_point(Point2::new(0.12, 0.08), 1.0);
        assert!((snapped.x - 0.12).abs() < 1e-6); // No snapping
    }
}

fn append_f64_prop<F>(parent: &Box, label: &str, value: f64, on_change: F)
where
    F: Fn(f64) + 'static,
{
    let row = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();
    row.append(&gtk4::Label::new(Some(label)));
    let entry = gtk4::Entry::builder()
        .text(&format!("{:.3}", value))
        .width_chars(8)
        .build();
    entry.connect_activate(move |e| {
        if let Ok(val) = e.text().parse::<f64>() {
            on_change(val);
        }
    });
    row.append(&entry);
    parent.append(&row);
}
