use glam::Vec2;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Cardinal {
    North,
    East,
    South,
    West,
}

pub(crate) struct RayCast {
    pub vec: Vec2,
    pub angle: f32,
    pub face_direction: Cardinal,
    pub hit_where: f32,
    pub tile: char,
}
