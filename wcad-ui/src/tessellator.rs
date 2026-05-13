use lyon::tessellation::*;
use lyon::path::Path;
use wcad_core::domain::{Entity, GeometryKind};
use crate::renderer::Vertex;
use nalgebra::Point2;
use wcad_core::domain::DimensionKind;

fn point(x: f32, y: f32) -> lyon::math::Point {
    lyon::math::point(x, y)
}

fn add_dashed_line(builder: &mut lyon::path::Builder, start: Point2<f64>, end: Point2<f64>, dash_len: f32, gap_len: f32) {
    let dir = (end - start).cast::<f32>();
    let total_len = dir.norm();
    if total_len < 1e-6 { return; }
    let dir_unit = dir / total_len;
    
    let mut current_dist = 0.0;
    let mut drawing = true;
    
    while current_dist < total_len {
        let step = if drawing { dash_len } else { gap_len };
        let next_dist = (current_dist + step).min(total_len);
        
        if drawing {
            let p1 = start.cast::<f32>() + dir_unit * current_dist;
            let p2 = start.cast::<f32>() + dir_unit * next_dist;
            builder.begin(point(p1.x, p1.y));
            builder.line_to(point(p2.x, p2.y));
            builder.end(false);
        }
        
        current_dist = next_dist;
        drawing = !drawing;
    }
}

fn add_arrowhead(builder: &mut lyon::path::Builder, tip: Point2<f64>, from: Point2<f64>, size: f32) {
    let dir = (tip - from).cast::<f32>();
    let len = dir.norm();
    if len < 1e-6 { return; }
    let unit = dir / len;
    let normal = nalgebra::Vector2::new(-unit.y, unit.x);
    
    let p1 = tip.cast::<f32>() - unit * size + normal * (size * 0.4);
    let p2 = tip.cast::<f32>() - unit * size - normal * (size * 0.4);
    
    builder.begin(point(p1.x, p1.y));
    builder.line_to(point(tip.x as f32, tip.y as f32));
    builder.line_to(point(p2.x, p2.y));
    builder.end(false);
}

pub fn tessellate_entities(entities: &[(&Entity, [f32; 3])], zoom: f32, height: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();
    
    // Maintain a minimum visual thickness of ~1.2 pixels
    let pixel_scale = 2.0 / (height * zoom);
    let line_width = (1.2 * pixel_scale).max(0.005);
    
    let options = StrokeOptions::default()
        .with_line_width(line_width)
        .with_line_cap(LineCap::Round);

    for entity in entities {
        let (geom, color) = entity;
        let mut builder = Path::builder();
        match &geom.geometry {
            GeometryKind::Point(p) => {
                let size = (1.0 * pixel_scale).max(0.003);
                builder.begin(point((p.x - size as f64) as f32, p.y as f32));
                builder.line_to(point((p.x + size as f64) as f32, p.y as f32));
                builder.end(false);
                builder.begin(point(p.x as f32, (p.y - size as f64) as f32));
                builder.line_to(point(p.x as f32, (p.y + size as f64) as f32));
                builder.end(false);
            }
            GeometryKind::Line { start, end } => {
                builder.begin(point(start.x as f32, start.y as f32));
                builder.line_to(point(end.x as f32, end.y as f32));
                builder.end(false);
            }
            GeometryKind::Circle { center, radius } => {
                let segments = 64;
                for i in 0..=segments {
                    let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                    let x = center.x as f32 + *radius as f32 * angle.cos();
                    let y = center.y as f32 + *radius as f32 * angle.sin();
                    if i == 0 {
                        builder.begin(point(x, y));
                    } else {
                        builder.line_to(point(x, y));
                    }
                }
                builder.end(true);
            }
            GeometryKind::Rectangle { start, end } => {
                let x1 = start.x as f32;
                let y1 = start.y as f32;
                let x2 = end.x as f32;
                let y2 = end.y as f32;
                builder.begin(point(x1, y1));
                builder.line_to(point(x2, y1));
                builder.line_to(point(x2, y2));
                builder.line_to(point(x1, y2));
                builder.end(true);
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                let segments = 64;
                for i in 0..=segments {
                    let t = i as f32 / segments as f32;
                    let angle = *start_angle as f32 + t * *sweep_angle as f32;
                    let x = center.x as f32 + *radius as f32 * angle.cos();
                    let y = center.y as f32 + *radius as f32 * angle.sin();
                    if i == 0 {
                        builder.begin(point(x, y));
                    } else {
                        builder.line_to(point(x, y));
                    }
                }
                builder.end(false);
            }
            GeometryKind::Polyline(points) => {
                if !points.is_empty() {
                    builder.begin(point(points[0].x as f32, points[0].y as f32));
                    for p in &points[1..] {
                        builder.line_to(point(p.x as f32, p.y as f32));
                    }
                    builder.end(false);
                }
            }
            GeometryKind::Dimension(dim) => {
                match dim {
                    DimensionKind::Linear { p1, p2, p_line, horizontal } => {
                        let (_, p1_dim, p2_dim) = if *horizontal {
                            ( (p2.x - p1.x).abs(), Point2::new(p1.x, p_line.y), Point2::new(p2.x, p_line.y) )
                        } else {
                            ( (p2.y - p1.y).abs(), Point2::new(p_line.x, p1.y), Point2::new(p_line.x, p2.y) )
                        };
                        let dash = (2.5 * pixel_scale).max(0.015) as f32;
                        let gap = (1.5 * pixel_scale).max(0.01) as f32;
                        let extension_overshoot = (2.5 * pixel_scale).max(0.015) as f64;
                        
                        // Extension lines (Dashed)
                        let dir1 = (p1_dim - p1).normalize();
                        let p1_ext = p1_dim + dir1 * extension_overshoot;
                        add_dashed_line(&mut builder, *p1, p1_ext, dash, gap);
                        
                        let dir2 = (p2_dim - p2).normalize();
                        let p2_ext = p2_dim + dir2 * extension_overshoot;
                        add_dashed_line(&mut builder, *p2, p2_ext, dash, gap);
                        
                        // Dim line (Solid)
                        builder.begin(point(p1_dim.x as f32, p1_dim.y as f32));
                        builder.line_to(point(p2_dim.x as f32, p2_dim.y as f32));
                        builder.end(false);
                        
                        // Arrowheads
                        let arrow_size = (3.0 * pixel_scale).max(0.015) as f32;
                        add_arrowhead(&mut builder, p1_dim, p2_dim, arrow_size);
                        add_arrowhead(&mut builder, p2_dim, p1_dim, arrow_size);
                    }
                    DimensionKind::Aligned { p1, p2, p_line } => {
                        let dir = (p2 - p1).normalize();
                        let normal = nalgebra::Vector2::new(-dir.y, dir.x);
                        let offset = (p_line - p1).dot(&normal);
                        let p1_dim = p1 + normal * offset;
                        let p2_dim = p2 + normal * offset;
                        let dash = (2.5 * pixel_scale).max(0.015) as f32;
                        let gap = (1.5 * pixel_scale).max(0.01) as f32;
                        let extension_overshoot = (2.5 * pixel_scale).max(0.015) as f64;

                        // Extension lines (Dashed)
                        let dir1 = (p1_dim - p1).normalize();
                        let p1_ext = p1_dim + dir1 * extension_overshoot;
                        add_dashed_line(&mut builder, *p1, p1_ext, dash, gap);
                        
                        let dir2 = (p2_dim - p2).normalize();
                        let p2_ext = p2_dim + dir2 * extension_overshoot;
                        add_dashed_line(&mut builder, *p2, p2_ext, dash, gap);
                        
                        // Dim line (Solid)
                        builder.begin(point(p1_dim.x as f32, p1_dim.y as f32));
                        builder.line_to(point(p2_dim.x as f32, p2_dim.y as f32));
                        builder.end(false);

                        // Arrowheads
                        let arrow_size = (3.0 * pixel_scale).max(0.015) as f32;
                        add_arrowhead(&mut builder, p1_dim, p2_dim, arrow_size);
                        add_arrowhead(&mut builder, p2_dim, p1_dim, arrow_size);
                    }
                    DimensionKind::Radial { center, point: p_on_circle, p_text } => {
                        // For radial, usually the leader is solid
                        builder.begin(point(center.x as f32, center.y as f32));
                        builder.line_to(point(p_on_circle.x as f32, p_on_circle.y as f32));
                        builder.end(false);
                        builder.begin(point(p_on_circle.x as f32, p_on_circle.y as f32));
                        builder.line_to(point(p_text.x as f32, p_text.y as f32));
                        builder.end(false);
                        
                        // Arrowhead at circle point
                        let arrow_size = (3.0 * pixel_scale).max(0.015) as f32;
                        add_arrowhead(&mut builder, *p_on_circle, *center, arrow_size);
                    }
                }
            }
        }
        
        let path = builder.build();
        tessellator.tessellate_path(
            &path,
            &options,
            &mut Builder {
                output: &mut geometry,
                color: *color,
            },
        ).unwrap();
    }
    
    (geometry.vertices, geometry.indices)
}

struct Builder<'a> {
    output: &'a mut VertexBuffers<Vertex, u32>,
    color: [f32; 3],
}

impl<'a> StrokeGeometryBuilder for Builder<'a> {
    fn add_stroke_vertex(&mut self, vertex: StrokeVertex) -> Result<VertexId, GeometryBuilderError> {
        let pos = vertex.position();
        let id = self.output.vertices.len() as u32;
        self.output.vertices.push(Vertex {
            position: [pos.x, pos.y],
            color: self.color,
        });
        Ok(VertexId(id))
    }
}

impl<'a> GeometryBuilder for Builder<'a> {
    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        self.output.indices.push(a.0);
        self.output.indices.push(b.0);
        self.output.indices.push(c.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Point2;

    #[test]
    fn test_tessellate_line() {
        let entity = Entity::new(GeometryKind::Line {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(1.0, 1.0),
        }, "0");
        let entities = vec![(&entity, [1.0, 1.0, 1.0])];
        let (vertices, indices) = tessellate_entities(&entities, 1.0, 800.0);
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        // A line stroke with thickness should have at least 4 vertices (a quad)
        assert!(vertices.len() >= 4);
        assert!(indices.len() >= 6);
    }

    #[test]
    fn test_tessellate_circle() {
        let entity = Entity::new(GeometryKind::Circle {
            center: Point2::new(0.0, 0.0),
            radius: 1.0,
        }, "0");
        let entities = vec![(&entity, [1.0, 1.0, 1.0])];
        let (vertices, indices) = tessellate_entities(&entities, 1.0, 800.0);
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        // A circle with 64 segments should have many more vertices than a line
        assert!(vertices.len() > 10);
    }
}
