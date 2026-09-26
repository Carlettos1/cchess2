use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Square orthogonal lattice. North increases y. Ordering is x, then y.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn offset(self, dx: i32, dy: i32) -> Option<Self> {
        Some(Self::new(self.x.checked_add(dx)?, self.y.checked_add(dy)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bounds {
    pub width: u16,
    pub height: u16,
}
impl Bounds {
    pub fn contains(self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < i32::from(self.width) && p.y < i32::from(self.height)
    }
    pub fn positions(self) -> impl Iterator<Item = Pos> {
        (0..i32::from(self.width))
            .flat_map(move |x| (0..i32::from(self.height)).map(move |y| Pos::new(x, y)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    N,
    E,
    S,
    W,
}
impl Direction {
    pub fn rotate(self, (x, y): (i32, i32)) -> (i32, i32) {
        match self {
            Self::N => (x, y),
            Self::E => (y, -x),
            Self::S => (-x, -y),
            Self::W => (-y, x),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubDirection {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}
impl SubDirection {
    pub const ALL: [Self; 8] = [
        Self::N,
        Self::NE,
        Self::E,
        Self::SE,
        Self::S,
        Self::SW,
        Self::W,
        Self::NW,
    ];
    pub fn vector(self) -> (i32, i32) {
        match self {
            Self::N => (0, 1),
            Self::NE => (1, 1),
            Self::E => (1, 0),
            Self::SE => (1, -1),
            Self::S => (0, -1),
            Self::SW => (-1, -1),
            Self::W => (-1, 0),
            Self::NW => (-1, 1),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relative {
    F,
    FR,
    R,
    BR,
    B,
    BL,
    L,
    FL,
}
impl Relative {
    pub fn absolute(self, forward: Direction) -> (i32, i32) {
        let vector = match self {
            Self::F => (0, 1),
            Self::FR => (1, 1),
            Self::R => (1, 0),
            Self::BR => (1, -1),
            Self::B => (0, -1),
            Self::BL => (-1, -1),
            Self::L => (-1, 0),
            Self::FL => (-1, 1),
        };
        forward.rotate(vector)
    }
}

/// Input syntax. Aliases disappear when the registry is compiled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Pattern {
    Any(Vec<Pattern>),
    SelfOnly,
    Ray {
        min: u16,
        max: Option<u16>,
        directions: Vec<SubDirection>,
    },
    InfiniteRay(Vec<SubDirection>),
    Step(Vec<SubDirection>),
    FullStep,
    Leap(Vec<(i16, i16)>),
    SymmetricalLeap(Vec<(u16, u16)>),
    DiagonalLeap(Vec<u16>),
    RelativeRay {
        min: u16,
        max: Option<u16>,
        directions: Vec<Relative>,
    },
    RelativeInfiniteRay(Vec<Relative>),
    RelativeStep(Vec<Relative>),
    EndOfBoard(Relative),
    TaxicabRange(u16),
    AreaSquare(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimePattern {
    Any(Vec<RuntimePattern>),
    SelfOnly,
    Ray {
        min: u16,
        max: Option<u16>,
        vectors: Vec<(i32, i32)>,
        relative: bool,
    },
    Leap(Vec<(i32, i32)>),
    EndOfBoard(Relative),
    Range {
        distance: u16,
        taxicab: bool,
    },
}
impl Pattern {
    pub fn normalize(&self) -> Result<RuntimePattern, String> {
        use RuntimePattern as R;
        let ray = |min: u16, max: Option<u16>, vectors: Vec<(i32, i32)>, relative| {
            if vectors.is_empty() || max.is_some_and(|m| m < min) {
                return Err("ray requires directions and min <= max".into());
            }
            Ok(R::Ray {
                min,
                max,
                vectors,
                relative,
            })
        };
        let leap = |vectors: Vec<(i32, i32)>| {
            if vectors.is_empty() {
                return Err("leap requires offsets".into());
            }
            Ok(R::Leap(
                vectors
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            ))
        };
        match self {
            Self::Any(patterns) => {
                if patterns.is_empty() {
                    return Err("Any requires at least one pattern".into());
                }
                patterns
                    .iter()
                    .map(Self::normalize)
                    .collect::<Result<Vec<_>, _>>()
                    .map(R::Any)
            }
            Self::SelfOnly => Ok(R::SelfOnly),
            Self::Ray {
                min,
                max,
                directions,
            } => ray(
                *min,
                *max,
                directions.iter().map(|d| d.vector()).collect(),
                false,
            ),
            Self::InfiniteRay(d) => ray(1, None, d.iter().map(|d| d.vector()).collect(), false),
            Self::Step(d) => ray(1, Some(1), d.iter().map(|d| d.vector()).collect(), false),
            Self::FullStep => leap(SubDirection::ALL.iter().map(|d| d.vector()).collect()),
            Self::Leap(v) => leap(
                v.iter()
                    .map(|&(x, y)| (i32::from(x), i32::from(y)))
                    .collect(),
            ),
            Self::SymmetricalLeap(v) => leap(
                v.iter()
                    .flat_map(|&(x, y)| {
                        let (x, y) = (i32::from(x), i32::from(y));
                        [
                            (x, y),
                            (y, x),
                            (-x, y),
                            (-y, x),
                            (x, -y),
                            (y, -x),
                            (-x, -y),
                            (-y, -x),
                        ]
                    })
                    .collect(),
            ),
            Self::DiagonalLeap(v) => leap(
                v.iter()
                    .flat_map(|&d| {
                        let d = i32::from(d);
                        [(d, d), (d, -d), (-d, d), (-d, -d)]
                    })
                    .collect(),
            ),
            Self::RelativeRay {
                min,
                max,
                directions,
            } => ray(
                *min,
                *max,
                directions
                    .iter()
                    .map(|d| d.absolute(Direction::N))
                    .collect(),
                true,
            ),
            Self::RelativeInfiniteRay(d) => ray(
                1,
                None,
                d.iter().map(|d| d.absolute(Direction::N)).collect(),
                true,
            ),
            Self::RelativeStep(d) => ray(
                1,
                Some(1),
                d.iter().map(|d| d.absolute(Direction::N)).collect(),
                true,
            ),
            Self::EndOfBoard(d) => Ok(R::EndOfBoard(*d)),
            Self::TaxicabRange(d) => Ok(R::Range {
                distance: *d,
                taxicab: true,
            }),
            Self::AreaSquare(d) => Ok(R::Range {
                distance: *d,
                taxicab: false,
            }),
        }
    }
}

impl RuntimePattern {
    /// Includes the first occupied ray square, never squares beyond it. Blockers
    /// before min still stop the ray. Filters decide whether that square is legal.
    pub fn targets(
        &self,
        bounds: Bounds,
        origin: Pos,
        forward: Direction,
        occupied: impl Fn(Pos) -> bool,
    ) -> Vec<Pos> {
        self.targets_inner(bounds, origin, forward, &occupied, &|_| true)
    }
    /// Resolve against a board mask as well as its rectangular envelope.
    pub fn targets_on_board(
        &self,
        bounds: Bounds,
        origin: Pos,
        forward: Direction,
        occupied: impl Fn(Pos) -> bool,
        playable: impl Fn(Pos) -> bool,
    ) -> Vec<Pos> {
        self.targets_inner(bounds, origin, forward, &occupied, &playable)
    }
    fn targets_inner(
        &self,
        bounds: Bounds,
        origin: Pos,
        forward: Direction,
        occupied: &dyn Fn(Pos) -> bool,
        playable: &dyn Fn(Pos) -> bool,
    ) -> Vec<Pos> {
        if !bounds.contains(origin) || !playable(origin) {
            return Vec::new();
        }
        let mut result = Vec::new();
        match self {
            Self::Any(patterns) => {
                for pattern in patterns {
                    result
                        .extend(pattern.targets_inner(bounds, origin, forward, occupied, playable));
                }
            }
            Self::SelfOnly => {
                result.push(origin);
            }
            Self::Leap(offsets) => {
                for &(dx, dy) in offsets {
                    if let Some(p) = origin
                        .offset(dx, dy)
                        .filter(|p| bounds.contains(*p) && playable(*p))
                    {
                        result.push(p);
                    }
                }
            }
            Self::Ray {
                min,
                max,
                vectors,
                relative,
            } => {
                if *min == 0 {
                    result.push(origin);
                }
                for &v in vectors {
                    let (dx, dy) = if *relative { forward.rotate(v) } else { v };
                    let limit = max.unwrap_or(bounds.width.max(bounds.height));
                    for distance in 1..=i32::from(limit) {
                        let Some(p) = origin
                            .offset(dx * distance, dy * distance)
                            .filter(|p| bounds.contains(*p) && playable(*p))
                        else {
                            break;
                        };
                        if distance >= i32::from(*min) {
                            result.push(p);
                        }
                        if occupied(p) {
                            break;
                        }
                    }
                }
            }
            Self::EndOfBoard(d) => {
                let (dx, dy) = d.absolute(forward);
                let mut p = origin;
                while let Some(next) = p
                    .offset(dx, dy)
                    .filter(|p| bounds.contains(*p) && playable(*p))
                {
                    p = next;
                }
                result.push(p);
            }
            Self::Range { distance, taxicab } => {
                // A local range need not scan the entire (up to 64x64) board.
                let radius = i32::from(*distance);
                let xs = (origin.x - radius).max(0)
                    ..=(origin.x + radius).min(i32::from(bounds.width) - 1);
                let ys = (origin.y - radius).max(0)
                    ..=(origin.y + radius).min(i32::from(bounds.height) - 1);
                for p in xs
                    .flat_map(|x| ys.clone().map(move |y| Pos::new(x, y)))
                    .filter(|p| playable(*p))
                {
                    let dx = (p.x - origin.x).abs();
                    let dy = (p.y - origin.y).abs();
                    if (if *taxicab { dx + dy } else { dx.max(dy) }) <= i32::from(*distance) {
                        result.push(p);
                    }
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }
}
