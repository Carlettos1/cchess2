use crate::{
    content::Tag,
    geometry::{Pos, RuntimePattern},
    state::{EntityId, Game, PieceInstance, PlayerId},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetOrigin {
    Source,
    AnyFriendlyPiece,
    AnyEnemyPiece,
    Fixed(Pos),
    Board,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetFilter {
    Empty,
    Magic,
    Buildable,
    Occupied,
    Transportable,
    Friendly,
    Enemy,
    HasTag(Tag),
    HasActed,
    Archetype(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec<P> {
    pub origin: TargetOrigin,
    pub pattern: P,
    pub filters: Vec<TargetFilter>,
    pub count: u16,
}

/// Shared selectable-target resolution, including deduplication across origins.
pub fn resolve(
    game: &Game,
    source: EntityId,
    owner: PlayerId,
    spec: &TargetSpec<RuntimePattern>,
) -> Vec<Pos> {
    let Some(player) = game.player(owner) else {
        return vec![];
    };
    let origins: Vec<_> = match &spec.origin {
        TargetOrigin::Source => game
            .piece(source)
            .map(|p| vec![p.position])
            .unwrap_or_default(),
        TargetOrigin::AnyFriendlyPiece => game
            .pieces()
            .filter(|p| p.owner == owner)
            .map(|p| p.position)
            .collect(),
        TargetOrigin::AnyEnemyPiece => game
            .pieces()
            .filter(|p| p.owner != owner)
            .map(|p| p.position)
            .collect(),
        TargetOrigin::Fixed(p) => vec![*p],
        TargetOrigin::Board => game.bounds().positions().collect(),
    };
    origins
        .into_iter()
        .flat_map(|origin| {
            spec.pattern.targets_on_board(
                game.bounds(),
                origin,
                player.forward,
                |p| game.at(p).is_some(),
                |p| game.is_playable(p),
            )
        })
        .filter(|pos| game.is_playable(*pos))
        .filter(|pos| {
            let piece = game.at(*pos);
            spec.filters
                .iter()
                .all(|f| matches_filter(game, piece, *pos, owner, f))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn matches_filter(
    game: &Game,
    piece: Option<&PieceInstance>,
    pos: Pos,
    owner: PlayerId,
    filter: &TargetFilter,
) -> bool {
    match filter {
        TargetFilter::Magic => game.state.magic_tiles.contains(&pos),
        TargetFilter::Buildable => !game.state.unbuildable_tiles.contains(&pos),
        TargetFilter::Empty => piece.is_none(),
        TargetFilter::Transportable => piece.is_some_and(|p| p.stats.transportable.is_some()),
        TargetFilter::Occupied => piece.is_some(),
        TargetFilter::Friendly => piece.is_some_and(|p| p.owner == owner),
        TargetFilter::Enemy => piece.is_some_and(|p| p.owner != owner),
        TargetFilter::HasTag(tag) => piece.is_some_and(|p| game.has_tag(p.id, *tag)),
        TargetFilter::HasActed => piece.is_some_and(|p| p.state.acted),
        TargetFilter::Archetype(id) => piece.is_some_and(|p| &p.archetype == id),
    }
}
