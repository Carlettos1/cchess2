//! Complete CChess armies, ported from the legacy board layout.
use crate::{
    cards::DeckDef,
    state::{Placement, Player},
    Bounds, Direction, Game, Pos, Registry,
};

fn army() -> Vec<(&'static str, i32, i32)> {
    let mut pieces = Vec::new();
    let back = [
        "cannon", "rook", "catapult", "knight", "warlock", "bishop", "magician", "queen", "king",
        "paladin", "bishop", "warlock", "knight", "catapult", "rook", "cannon",
    ];
    let second = [
        "ship",
        "tesla_tower",
        "ram",
        "builder",
        "pawn",
        "pawn",
        "crazy_pawn",
        "super_pawn",
        "super_pawn",
        "crazy_pawn",
        "pawn",
        "pawn",
        "builder",
        "ram",
        "tesla_tower",
        "ship",
    ];
    for (y, row) in [back, second].into_iter().enumerate() {
        for (x, name) in row.into_iter().enumerate() {
            pieces.push((name, x as i32, y as i32));
        }
    }
    for (x, name) in ["ballista", "archer", "archer", "shield_bearer"]
        .into_iter()
        .enumerate()
    {
        pieces.push((name, x as i32, 2));
        pieces.push((name, 15 - x as i32, 2));
    }
    for (x, name) in ["pawn", "crazy_pawn", "pawn"].into_iter().enumerate() {
        pieces.push((name, x as i32, 3));
        pieces.push((name, 15 - x as i32, 3));
    }
    pieces
}
fn rotate(pos: Pos, owner: u8) -> Pos {
    match owner {
        0 => pos,
        1 => Pos::new(pos.y, 23 - pos.x),
        2 => Pos::new(23 - pos.x, 23 - pos.y),
        _ => Pos::new(23 - pos.y, pos.x),
    }
}
impl Game {
    /// Two complete 46-piece armies on the original 16x17 board, or four
    /// rotated armies on a 24x24 cross with four 4x4 corners removed.
    pub fn full_board(registry: Registry, four_player: bool) -> Result<Self, String> {
        let count = if four_player { 4 } else { 2 };
        let forwards = if four_player {
            vec![Direction::N, Direction::E, Direction::S, Direction::W]
        } else {
            vec![Direction::N, Direction::S]
        };
        let players = forwards
            .into_iter()
            .enumerate()
            .map(|(id, d)| Player::new(id as u8, d))
            .collect();
        let mut placements = Vec::new();
        for owner in 0..count {
            for (name, x, y) in army() {
                let pos = if four_player {
                    rotate(Pos::new(x + 4, y), owner)
                } else {
                    Pos::new(x, if owner == 0 { y } else { 16 - y })
                };
                placements.push(Placement::new(name, owner, pos));
            }
        }
        let magic = if four_player {
            (0..4)
                .flat_map(|owner| {
                    [
                        rotate(Pos::new(4, 7), owner),
                        rotate(Pos::new(19, 7), owner),
                    ]
                })
                .collect()
        } else {
            vec![
                Pos::new(0, 7),
                Pos::new(0, 9),
                Pos::new(15, 7),
                Pos::new(15, 9),
            ]
        };
        let bounds = if four_player {
            Bounds {
                width: 24,
                height: 24,
            }
        } else {
            Bounds {
                width: 16,
                height: 17,
            }
        };
        let deck: DeckDef = ron::from_str(include_str!("../../assets/decks/starter.ron"))
            .map_err(|e| e.to_string())?;
        Self::new(registry, bounds, players, placements)?
            .with_terrain(magic, vec![])?
            .with_decks(
                (0..count).map(|owner| (owner, deck.clone())).collect(),
                42,
                3,
                1,
                true,
            )
    }
}
