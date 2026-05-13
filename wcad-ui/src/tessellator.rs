use lyon::tessellation::*;
use lyon::path::Path;
use lyon::math::point;
use wcad_core::domain::Entity;
use crate::renderer::Vertex;

pub fn tessellate_entities(entities: &[Entity]) -> (Vec<Vertex>, Vec<u32>) {
    let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();
    let options = StrokeOptions::default().with_line_width(0.005);

    for entity in entities {
        let mut builder = Path::builder();
        match entity {
            Entity::Point(p) => {
                let size = 0.005f32;
                builder.begin(point((p.x - size as f64) as f32, p.y as f32));
                builder.line_to(point((p.x + size as f64) as f32, p.y as f32));
                builder.end(false);
                builder.begin(point(p.x as f32, (p.y - size as f64) as f32));
                builder.line_to(point(p.x as f32, (p.y + size as f64) as f32));
                builder.end(false);
            }
            Entity::Line { start, end } => {
                builder.begin(point(start.x as f32, start.y as f32));
                builder.line_to(point(end.x as f32, end.y as f32));
                builder.end(false);
            }
            Entity::Circle { center, radius } => {
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
        }
        
        let path = builder.build();
        tessellator.tessellate_path(
            &path,
            &options,
            &mut Builder {
                output: &mut geometry,
            },
        ).unwrap();
    }
    
    (geometry.vertices, geometry.indices)
}

struct Builder<'a> {
    output: &'a mut VertexBuffers<Vertex, u32>,
}

impl<'a> StrokeGeometryBuilder for Builder<'a> {
    fn add_stroke_vertex(&mut self, vertex: StrokeVertex) -> Result<VertexId, GeometryBuilderError> {
        let pos = vertex.position();
        let id = self.output.vertices.len() as u32;
        self.output.vertices.push(Vertex {
            position: [pos.x, pos.y],
            color: [1.0, 1.0, 1.0],
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
        let entities = vec![
            Entity::Line {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(1.0, 1.0),
            }
        ];
        let (vertices, indices) = tessellate_entities(&entities);
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        // A line stroke with thickness should have at least 4 vertices (a quad)
        assert!(vertices.len() >= 4);
        assert!(indices.len() >= 6);
    }

    #[test]
    fn test_tessellate_circle() {
        let entities = vec![
            Entity::Circle {
                center: Point2::new(0.0, 0.0),
                radius: 1.0,
            }
        ];
        let (vertices, indices) = tessellate_entities(&entities);
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());
        // A circle with 64 segments should have many more vertices than a line
        assert!(vertices.len() > 10);
    }
}
