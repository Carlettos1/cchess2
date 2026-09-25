#![allow(dead_code)]
use cchess_engine::{
    cards::{CardChoice, CardDef, DeckDef, CARD_SOURCES},
    content::{PieceArchetype, CLASSICAL_SOURCES, CUSTOM_SOURCES},
    state::{Placement, Player},
    *,
};
pub fn registry(edit: impl FnOnce(&mut Vec<PieceArchetype>, &mut Vec<CardDef>)) -> Registry {
    let mut pieces: Vec<PieceArchetype> = CLASSICAL_SOURCES
        .iter()
        .chain(CUSTOM_SOURCES.iter())
        .map(|(_, s)| ron::from_str(s).unwrap())
        .collect();
    let mut cards: Vec<CardDef> = CARD_SOURCES
        .iter()
        .map(|(_, s)| ron::from_str(s).unwrap())
        .collect();
    edit(&mut pieces, &mut cards);
    let ps: Vec<_> = pieces
        .iter()
        .map(|p| (p.id.clone(), ron::to_string(p).unwrap()))
        .collect();
    let cs: Vec<_> = cards
        .iter()
        .map(|c| (c.id.clone(), ron::to_string(c).unwrap()))
        .collect();
    Registry::load(ps.iter().map(|(id, s)| (id.as_str(), s.as_str())))
        .unwrap()
        .with_cards(cs.iter().map(|(id, s)| (id.as_str(), s.as_str())))
        .unwrap()
}
pub fn deck(names: &[&str]) -> DeckDef {
    DeckDef {
        schema_version: 1,
        id: "test".into(),
        cards: names.iter().map(|s| s.to_string()).collect(),
    }
}
pub fn setup(
    reg: Registry,
    pieces: &[(&str, u8, i32, i32)],
    white: &[&str],
    black: &[&str],
    opening: u32,
    draw: u32,
) -> Game {
    let players = [Player::new(0, Direction::N), Player::new(1, Direction::S)]
        .into_iter()
        .map(|mut p| {
            p.mana = 20.into();
            p.mana_per_turn = 0;
            p
        })
        .collect();
    Game::new(
        reg,
        Bounds {
            width: 8,
            height: 8,
        },
        players,
        pieces
            .iter()
            .map(|&(id, owner, x, y)| Placement::new(id, owner, Pos::new(x, y)))
            .collect(),
    )
    .unwrap()
    .with_decks(
        vec![(0, deck(white)), (1, deck(black))],
        17,
        opening,
        draw,
        false,
    )
    .unwrap()
}
pub fn game(pieces: &[(&str, u8, i32, i32)], white: &[&str], black: &[&str], opening: u32) -> Game {
    setup(
        Registry::built_in().unwrap(),
        pieces,
        white,
        black,
        opening,
        0,
    )
}
pub fn id(g: &Game, x: i32, y: i32) -> u64 {
    g.at(Pos::new(x, y)).unwrap().id
}
pub fn hand(g: &Game, name: &str) -> u64 {
    g.player(g.turn().player)
        .unwrap()
        .cards
        .hand
        .iter()
        .copied()
        .find(|id| g.card_instance(*id).unwrap().definition == name)
        .unwrap()
}
pub fn play(g: &mut Game, name: &str, target: CardChoice) -> ResolvedAction {
    let card = hand(g, name);
    g.apply(ActionIntent::PlayCard {
        player: g.turn().player,
        card,
        target,
    })
    .unwrap()
}
pub fn pass(g: &mut Game) -> ResolvedAction {
    g.apply(ActionIntent::EndTurn {
        player: g.turn().player,
    })
    .unwrap()
}
pub fn act(g: &mut Game, source: u64, kind: ActionKind, target: Pos) -> ResolvedAction {
    g.apply(ActionIntent::Piece {
        player: g.turn().player,
        source,
        kind,
        target,
    })
    .unwrap()
}
pub fn cast(g: &mut Game, source: u64, ability: &str, target: Option<Pos>) -> ResolvedAction {
    g.apply(ActionIntent::Ability {
        player: g.turn().player,
        source,
        ability: ability.into(),
        target,
    })
    .unwrap()
}
