use nalgebra::Point2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entity {
    Point(Point2<f64>),
    Line { start: Point2<f64>, end: Point2<f64> },
    Circle { center: Point2<f64>, radius: f64 },
}

pub trait Geometry {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>);
}

impl Geometry for Entity {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>) {
        match self {
            Entity::Point(p) => (*p, *p),
            Entity::Line { start, end } => {
                let min_x = start.x.min(end.x);
                let min_y = start.y.min(end.y);
                let max_x = start.x.max(end.x);
                let max_y = start.y.max(end.y);
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            Entity::Circle { center, radius } => {
                let min = Point2::new(center.x - radius, center.y - radius);
                let max = Point2::new(center.x + radius, center.y + radius);
                (min, max)
            }
        }
    }
}
