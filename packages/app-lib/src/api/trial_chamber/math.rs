//! Block positions, directions and rotations, mirroring Minecraft's own math
//! types.
//!
//! Trial chamber layout is decided by a jigsaw algorithm whose random draws have
//! to happen in exactly the same order as vanilla's, so these types keep the same
//! integer semantics rather than the idiomatic Rust ones: a rotation is an index
//! into a fixed list, and a direction knows both its id and its step vector.

use serde::{Deserialize, Serialize};

/// A block position; the equivalent of `net.minecraft.core.Vec3i`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Pack a position into one `i64` the way vanilla does, used as a set key.
    pub const fn as_long(self) -> i64 {
        // 38 bits of x, 12 of z, 26 of y — the same split as `BlockPos.asLong`.
        ((self.x as i64 & 0x3FF_FFFF) << 38)
            | ((self.z as i64 & 0xFFF) << 26)
            | (self.y as i64 & 0x3FF_FFFF)
    }

    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub const fn relative(self, direction: Direction) -> Self {
        self.offset(direction.step_x(), direction.step_y(), direction.step_z())
    }

    pub const fn subtract(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

/// A block position carrying a direction, used by jigsaw connectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockPosWithDirection {
    pub pos: BlockPos,
    pub direction: Direction,
}

/// The six axis directions, in vanilla's declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    /// Declaration order, which is also the id order.
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    /// The three axes.
    pub const HORIZONTAL: [Direction; 4] = [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    pub const fn id(self) -> i32 {
        match self {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::North => 2,
            Direction::South => 3,
            Direction::West => 4,
            Direction::East => 5,
        }
    }

    pub const fn step_x(self) -> i32 {
        match self {
            Direction::East => 1,
            Direction::West => -1,
            _ => 0,
        }
    }

    pub const fn step_y(self) -> i32 {
        match self {
            Direction::Up => 1,
            Direction::Down => -1,
            _ => 0,
        }
    }

    pub const fn step_z(self) -> i32 {
        match self {
            Direction::South => 1,
            Direction::North => -1,
            _ => 0,
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    pub const fn axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

    /// The two directions on the plane perpendicular to `axis`.
    pub fn plane(axis: Axis) -> [Direction; 2] {
        match axis {
            Axis::X => [Direction::East, Direction::West],
            Axis::Y => [Direction::Up, Direction::Down],
            Axis::Z => [Direction::South, Direction::North],
        }
    }

    pub fn by_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|direction| direction.id() == id)
    }

    pub const fn serialized_name(self) -> &'static str {
        match self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::North => "north",
            Direction::South => "south",
            Direction::West => "west",
            Direction::East => "east",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const fn is_horizontal(self) -> bool {
        !matches!(self, Axis::Y)
    }
}

/// The four horizontal rotations, in vanilla's declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

impl Rotation {
    pub const ALL: [Rotation; 4] = [
        Rotation::None,
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::CounterClockwise90,
    ];

    pub const fn index(self) -> i32 {
        match self {
            Rotation::None => 0,
            Rotation::Clockwise90 => 1,
            Rotation::Clockwise180 => 2,
            Rotation::CounterClockwise90 => 3,
        }
    }

    pub fn by_index(index: i32) -> Self {
        Self::ALL
            .into_iter()
            .find(|rotation| rotation.index() == index)
            .unwrap_or(Rotation::None)
    }

    /// Rotate a direction. Vertical directions are unaffected.
    pub const fn rotate(self, direction: Direction) -> Direction {
        match self {
            Rotation::None => direction,
            Rotation::Clockwise90 => match direction {
                Direction::North => Direction::East,
                Direction::East => Direction::South,
                Direction::South => Direction::West,
                Direction::West => Direction::North,
                other => other,
            },
            Rotation::Clockwise180 => match direction {
                Direction::North => Direction::South,
                Direction::South => Direction::North,
                Direction::East => Direction::West,
                Direction::West => Direction::East,
                other => other,
            },
            Rotation::CounterClockwise90 => match direction {
                Direction::North => Direction::West,
                Direction::West => Direction::South,
                Direction::South => Direction::East,
                Direction::East => Direction::North,
                other => other,
            },
        }
    }
}

/// A chunk column position, in chunks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// The chunk containing a block position, matching vanilla's floor division
    /// for negatives.
    pub const fn from_block(pos: BlockPos) -> Self {
        Self::new(pos.x.div_euclid(16), pos.z.div_euclid(16))
    }

    pub const fn to_long(self) -> i64 {
        ((self.x as i64) & 0xFFFF_FFFF) | (((self.z as i64) & 0xFFFF_FFFF) << 32)
    }
}

/// An axis-aligned box, inclusive on both ends like vanilla's `BoundingBox`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundingBox {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

impl BoundingBox {
    pub const fn new(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// The box covering a structure piece of `size` starting at `origin`.
    pub const fn from_size(origin: BlockPos, size_x: i32, size_y: i32, size_z: i32) -> Self {
        Self::new(
            origin.x,
            origin.y,
            origin.z,
            origin.x + size_x - 1,
            origin.y + size_y - 1,
            origin.z + size_z - 1,
        )
    }

    pub const fn size_x(self) -> i32 {
        self.max_x - self.min_x + 1
    }

    pub const fn size_y(self) -> i32 {
        self.max_y - self.min_y + 1
    }

    pub const fn size_z(self) -> i32 {
        self.max_z - self.min_z + 1
    }

    /// Whether two boxes overlap. Vanilla's jigsaw treats touching faces as
    /// separate, so the comparison is strict.
    pub const fn intersects(self, other: Self) -> bool {
        self.max_x >= other.min_x
            && self.min_x <= other.max_x
            && self.max_y >= other.min_y
            && self.min_y <= other.max_y
            && self.max_z >= other.min_z
            && self.min_z <= other.max_z
    }

    pub const fn moved(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(
            self.min_x + dx,
            self.min_y + dy,
            self.min_z + dz,
            self.max_x + dx,
            self.max_y + dy,
            self.max_z + dz,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_and_orders_positions() {
        let pos = BlockPos::new(16, 64, -32);
        assert_eq!(pos.as_long() & 0xFFF, pos.z as i64 & 0xFFF);
    }

    #[test]
    fn rotates_horizontal_directions_only() {
        assert_eq!(Rotation::Clockwise90.rotate(Direction::North), Direction::East);
        assert_eq!(Rotation::Clockwise180.rotate(Direction::North), Direction::South);
        assert_eq!(
            Rotation::CounterClockwise90.rotate(Direction::East),
            Direction::North
        );
        assert_eq!(Rotation::Clockwise90.rotate(Direction::Up), Direction::Up);
    }

    #[test]
    fn chunk_from_block_floors_towards_negative_infinity() {
        assert_eq!(ChunkPos::from_block(BlockPos::new(-1, 0, -16)), ChunkPos::new(-1, -1));
        assert_eq!(ChunkPos::from_block(BlockPos::new(15, 0, 16)), ChunkPos::new(0, 1));
    }

    #[test]
    fn boxes_touching_faces_do_not_intersect() {
        let a = BoundingBox::new(0, 0, 0, 1, 1, 1);
        let b = BoundingBox::new(2, 0, 0, 3, 1, 1);
        assert!(!a.intersects(b));

        let c = BoundingBox::new(1, 0, 0, 2, 1, 1);
        assert!(a.intersects(c));
    }
}
