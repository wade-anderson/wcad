use std::io::Cursor;
use nalgebra::Point2;
use svg::Document;
use svg::node::element::{Line, Circle, Path, Rectangle};
use svg::node::element::path::Data;
use dxf::Drawing;
use dxf::entities::{Entity as DxfEntity, Line as DxfLine, Circle as DxfCircle, Arc as DxfArc, LwPolyline, EntityType};
use dxf::Point;
use base64::prelude::*;
use gtk4::cairo::{ImageSurface, Format, Context};
use wcad_core::domain::{Entity, GeometryKind, DimensionKind, Geometry};

pub fn export_svg(entities: &[Entity]) -> String {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for entity in entities {
        let bbox = entity.geometry.bounding_box();
        min_x = min_x.min(bbox.0.x);
        min_y = min_y.min(bbox.0.y);
        max_x = max_x.max(bbox.1.x);
        max_y = max_y.max(bbox.1.y);
    }

    if min_x == f64::MAX {
        min_x = 0.0; min_y = 0.0; max_x = 100.0; max_y = 100.0;
    }

    let padding = ((max_x - min_x).max(max_y - min_y) * 0.1).max(1.0);
    let mut document = Document::new()
        .set("viewBox", (
            min_x - padding, 
            min_y - padding, 
            (max_x - min_x) + 2.0 * padding, 
            (max_y - min_y) + 2.0 * padding
        ));

    for entity in entities {
        let layer_color = "black";
        match &entity.geometry {
            GeometryKind::Point(p) => {
                let circle = Circle::new()
                    .set("cx", p.x)
                    .set("cy", p.y)
                    .set("r", 0.5)
                    .set("fill", layer_color);
                document = document.add(circle);
            }
            GeometryKind::Line { start, end } => {
                let line = Line::new()
                    .set("x1", start.x)
                    .set("y1", start.y)
                    .set("x2", end.x)
                    .set("y2", end.y)
                    .set("stroke", layer_color)
                    .set("stroke-width", 0.2);
                document = document.add(line);
            }
            GeometryKind::Circle { center, radius } => {
                let circle = Circle::new()
                    .set("cx", center.x)
                    .set("cy", center.y)
                    .set("r", *radius)
                    .set("fill", "none")
                    .set("stroke", layer_color)
                    .set("stroke-width", 0.2);
                document = document.add(circle);
            }
            GeometryKind::Rectangle { start, end } => {
                let width = (end.x - start.x).abs();
                let height = (end.y - start.y).abs();
                let x = start.x.min(end.x);
                let y = start.y.min(end.y);
                let rect = Rectangle::new()
                    .set("x", x)
                    .set("y", y)
                    .set("width", width)
                    .set("height", height)
                    .set("fill", "none")
                    .set("stroke", layer_color)
                    .set("stroke-width", 0.2);
                document = document.add(rect);
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                let start_x = center.x + radius * start_angle.cos();
                let start_y = center.y + radius * start_angle.sin();
                let end_x = center.x + radius * (start_angle + sweep_angle).cos();
                let end_y = center.y + radius * (start_angle + sweep_angle).sin();

                let large_arc_flag = if sweep_angle.abs() > std::f64::consts::PI { 1 } else { 0 };
                let sweep_flag = if *sweep_angle > 0.0 { 1 } else { 0 };

                let data = Data::new()
                    .move_to((start_x, start_y))
                    .elliptical_arc_to((
                        *radius, *radius, 0, large_arc_flag, sweep_flag, end_x, end_y
                    ));
                
                let path = Path::new()
                    .set("d", data)
                    .set("fill", "none")
                    .set("stroke", layer_color)
                    .set("stroke-width", 0.2);
                document = document.add(path);
            }
            GeometryKind::Polyline(points) => {
                if points.len() < 2 { continue; }
                let mut data = Data::new().move_to((points[0].x, points[0].y));
                for p in &points[1..] {
                    data = data.line_to((p.x, p.y));
                }
                let path = Path::new()
                    .set("d", data)
                    .set("fill", "none")
                    .set("stroke", layer_color)
                    .set("stroke-width", 0.2);
                document = document.add(path);
            }
            GeometryKind::Image { top_left, bottom_right, data, format } => {
                let width = (bottom_right.x - top_left.x).abs();
                let height = (bottom_right.y - top_left.y).abs();
                let x = top_left.x.min(bottom_right.x);
                let y = top_left.y.min(bottom_right.y);
                
                let base64_data = BASE64_STANDARD.encode(data);
                let href = format!("data:image/{};base64,{}", format, base64_data);
                
                let image = svg::node::element::Image::new()
                    .set("x", x)
                    .set("y", y)
                    .set("width", width)
                    .set("height", height)
                    .set("xlink:href", href);
                document = document.add(image);
            }
            GeometryKind::Dimension(dim) => {
                 match dim {
                    DimensionKind::Linear { p1, p2: _, p_line, .. } | 
                    DimensionKind::Aligned { p1, p2: _, p_line, .. } => {
                        let l1 = Line::new().set("x1", p1.x).set("y1", p1.y).set("x2", p_line.x).set("y2", p_line.y).set("stroke", "gray").set("stroke-width", 0.1);
                        document = document.add(l1);
                    }
                    _ => {}
                }
            }
        }
    }

    document.to_string()
}

pub fn export_dxf(entities: &[Entity]) -> Vec<u8> {
    let mut drawing = Drawing::new();
    for entity in entities {
        match &entity.geometry {
            GeometryKind::Point(p) => {
                drawing.add_entity(DxfEntity {
                    common: Default::default(),
                    specific: dxf::entities::EntityType::Circle(dxf::entities::Circle::new(dxf::Point::new(p.x, p.y, 0.0), 0.1)),
                });
            }
            GeometryKind::Line { start, end } => {
                drawing.add_entity(DxfEntity {
                    common: Default::default(),
                    specific: dxf::entities::EntityType::Line(DxfLine::new(
                        Point::new(start.x, start.y, 0.0),
                        Point::new(end.x, end.y, 0.0)
                    )),
                });
            }
            GeometryKind::Circle { center, radius } => {
                drawing.add_entity(DxfEntity {
                    common: Default::default(),
                    specific: dxf::entities::EntityType::Circle(DxfCircle::new(
                        Point::new(center.x, center.y, 0.0),
                        *radius
                    )),
                });
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                 drawing.add_entity(DxfEntity {
                    common: Default::default(),
                    specific: dxf::entities::EntityType::Arc(DxfArc::new(
                        Point::new(center.x, center.y, 0.0),
                        *radius,
                        start_angle.to_degrees(),
                        (start_angle + sweep_angle).to_degrees()
                    )),
                });
            }
            GeometryKind::Rectangle { start, end } => {
                let mut lw = LwPolyline::default();
                // LwPolyline flags
                lw.vertices.push(dxf::LwPolylineVertex { x: start.x, y: start.y, ..Default::default() });
                lw.vertices.push(dxf::LwPolylineVertex { x: end.x, y: start.y, ..Default::default() });
                lw.vertices.push(dxf::LwPolylineVertex { x: end.x, y: end.y, ..Default::default() });
                lw.vertices.push(dxf::LwPolylineVertex { x: start.x, y: end.y, ..Default::default() });
                drawing.add_entity(DxfEntity {
                    common: Default::default(),
                    specific: dxf::entities::EntityType::LwPolyline(lw),
                });
            }
            GeometryKind::Polyline(points) => {
                let mut lw = LwPolyline::default();
                for p in points {
                    lw.vertices.push(dxf::LwPolylineVertex { x: p.x, y: p.y, ..Default::default() });
                }
                drawing.add_entity(DxfEntity {
                    common: Default::default(),
                    specific: dxf::entities::EntityType::LwPolyline(lw),
                });
            }
            _ => {}
        }
    }
    let mut buffer = Vec::new();
    drawing.save(&mut buffer).unwrap();
    buffer
}

pub fn export_png(entities: &[Entity], width: i32, height: i32) -> Vec<u8> {
    let surface = ImageSurface::create(Format::ARgb32, width, height).expect("Failed to create surface");
    let cr = Context::new(&surface).expect("Failed to create context");

    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.paint().unwrap();

    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for entity in entities {
        let bbox = entity.geometry.bounding_box();
        min_x = min_x.min(bbox.0.x);
        min_y = min_y.min(bbox.0.y);
        max_x = max_x.max(bbox.1.x);
        max_y = max_y.max(bbox.1.y);
    }

    if min_x != f64::MAX {
        let drawing_w = (max_x - min_x).max(1.0);
        let drawing_h = (max_y - min_y).max(1.0);
        let scale = (width as f64 / drawing_w).min(height as f64 / drawing_h) * 0.9;
        
        cr.translate(width as f64 / 2.0, height as f64 / 2.0);
        cr.scale(scale, -scale);
        cr.translate(-(min_x + max_x) / 2.0, -(min_y + max_y) / 2.0);
    }

    cr.set_source_rgb(0.0, 0.0, 0.0);
    cr.set_line_width(0.5);

    for entity in entities {
        match &entity.geometry {
            GeometryKind::Line { start, end } => {
                cr.move_to(start.x, start.y);
                cr.line_to(end.x, end.y);
                cr.stroke().unwrap();
            }
            GeometryKind::Circle { center, radius } => {
                cr.arc(center.x, center.y, *radius, 0.0, 2.0 * std::f64::consts::PI);
                cr.stroke().unwrap();
            }
            GeometryKind::Rectangle { start, end } => {
                let x = start.x.min(end.x);
                let y = start.y.min(end.y);
                let w = (end.x - start.x).abs();
                let h = (end.y - start.y).abs();
                cr.rectangle(x, y, w, h);
                cr.stroke().unwrap();
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                if *sweep_angle > 0.0 {
                    cr.arc(center.x, center.y, *radius, *start_angle, start_angle + sweep_angle);
                } else {
                    cr.arc_negative(center.x, center.y, *radius, *start_angle, start_angle + sweep_angle);
                }
                cr.stroke().unwrap();
            }
            GeometryKind::Polyline(points) => {
                if points.len() < 2 { continue; }
                cr.move_to(points[0].x, points[0].y);
                for p in &points[1..] {
                    cr.line_to(p.x, p.y);
                }
                cr.stroke().unwrap();
            }
            _ => {}
        }
    }

    let mut buffer = Vec::new();
    surface.write_to_png(&mut buffer).unwrap();
    buffer
}

pub fn import_dxf(data: &[u8]) -> Vec<GeometryKind> {
    let mut cursor = Cursor::new(data);
    let drawing = Drawing::load(&mut cursor).expect("Failed to load DXF");
    let mut geoms = Vec::new();

    for e in drawing.entities() {
        match &e.specific {
            EntityType::Line(l) => {
                geoms.push(GeometryKind::Line {
                    start: Point2::new(l.p1.x, l.p1.y),
                    end: Point2::new(l.p2.x, l.p2.y),
                });
            }
            EntityType::Circle(c) => {
                geoms.push(GeometryKind::Circle {
                    center: Point2::new(c.center.x, c.center.y),
                    radius: c.radius,
                });
            }
            EntityType::Arc(a) => {
                geoms.push(GeometryKind::Arc {
                    center: Point2::new(a.center.x, a.center.y),
                    radius: a.radius,
                    start_angle: a.start_angle.to_radians(),
                    sweep_angle: (a.end_angle - a.start_angle).to_radians(),
                });
            }
            EntityType::LwPolyline(lw) => {
                let pts: Vec<_> = lw.vertices.iter().map(|v| Point2::new(v.x, v.y)).collect();
                geoms.push(GeometryKind::Polyline(pts));
            }
            _ => {}
        }
    }
    geoms
}

#[cfg(test)]
mod tests {
    use super::*;
    use wcad_core::domain::Entity;
    use nalgebra::Point2;

    #[test]
    fn test_svg_export_basic() {
        let entities = vec![
            Entity::new(1, GeometryKind::Line {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(100.0, 100.0),
            }, "0"),
        ];
        let svg = export_svg(&entities);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("x1=\"0\""));
        assert!(svg.contains("x2=\"100\""));
    }

    #[test]
    fn test_dxf_roundtrip_basic() {
        let entities = vec![
            Entity::new(1, GeometryKind::Line {
                start: Point2::new(10.0, 20.0),
                end: Point2::new(30.0, 40.0),
            }, "0"),
            Entity::new(2, GeometryKind::Circle {
                center: Point2::new(50.0, 60.0),
                radius: 15.0,
            }, "0"),
        ];
        let dxf_data = export_dxf(&entities);
        let imported = import_dxf(&dxf_data);
        
        assert_eq!(imported.len(), 2);
        
        if let GeometryKind::Line { start, end } = &imported[0] {
            assert_eq!(start.x, 10.0);
            assert_eq!(end.y, 40.0);
        } else {
            panic!("First element should be a line");
        }
        
        if let GeometryKind::Circle { center, radius } = &imported[1] {
            assert_eq!(center.x, 50.0);
            assert_eq!(radius, &15.0);
        } else {
            panic!("Second element should be a circle");
        }
    }
}
