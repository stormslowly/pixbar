#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    Empty,
    PrimaryFull,
    SecondaryFull,
    PrimaryBoundary,
    SecondaryBoundary,
    DegradedOverlap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub kind: CellKind,
    pub sub_fill: u8,
}
