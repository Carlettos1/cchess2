//! Single-threaded, seeded UCT with progressive widening and bounded rollouts.
//! Simulations use the actual game state (including hidden information/RNG).
use crate::{heuristic, GameAi};
use cchess_core::{
    cards::MatchRng, content::Tag, simulation::Simulation, ActionIntent, ActionKind, Game,
};

/// Work limits count actions, not alternating turns. No threads or wall-clock
/// cutoffs are used, so the same position, seed and budget reproduce a choice.
#[derive(Debug, Clone, Copy)]
pub struct SearchConfig {
    pub iterations: u32,
    pub rollout_depth: u32,
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            iterations: 128,
            rollout_depth: 8,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub action: Option<ActionIntent>,
    pub iterations: u32,
    pub nodes: usize,
    pub rollout_actions: u64,
    pub failed_actions: u64,
    pub max_tree_depth: usize,
}

struct Node {
    game: Simulation,
    action: Option<ActionIntent>,
    actor: usize,
    children: Vec<usize>,
    untried: Option<Vec<ActionIntent>>,
    visits: u32,
    rewards: [f64; 4],
}
impl Node {
    fn new(game: Simulation, action: Option<ActionIntent>) -> Self {
        let actor = game
            .players()
            .iter()
            .position(|p| p.id == game.turn().player)
            .unwrap();
        Self {
            game,
            action,
            actor,
            children: Vec::new(),
            untried: None,
            visits: 0,
            rewards: [0.0; 4],
        }
    }
}

/// Search any active position; controller configuration supplies a seed when
/// present. Zero iterations or a terminal position returns no action.
pub fn search(game: &Game, config: SearchConfig) -> SearchResult {
    let mut result = SearchResult::default();
    if game.is_over() || config.iterations == 0 {
        return result;
    }
    let root_player = game.turn().player;
    let seed = game
        .ai_controller(root_player)
        .map_or(0, |c| c.seed)
        .wrapping_add(game.action_count().wrapping_mul(0xd1342543de82ef95));
    let mut rng = MatchRng::new(seed);
    let baseline = heuristic::evaluate_all(game);
    let mut tree = vec![Node::new(game.simulation(), None)];
    let mut path = Vec::new();
    for _ in 0..config.iterations {
        path.clear();
        path.push(0);
        let mut index = 0;
        loop {
            if tree[index].game.is_over() {
                break;
            }
            if tree[index].untried.is_none() {
                tree[index].untried = Some(ordered_actions(&tree[index].game, &mut rng));
            }
            // Large card/transport branching factors must not consume the entire
            // budget on root expansion. The permitted width grows with visits.
            let width = (2.0 * f64::from(tree[index].visits + 1).sqrt()) as usize;
            let can_expand = tree[index].children.len() < width;
            if can_expand {
                if let Some(action) = tree[index].untried.as_mut().unwrap().pop() {
                    let mut next = tree[index].game.clone();
                    if next.apply(action.clone()).is_err() {
                        result.failed_actions += 1;
                        continue;
                    }
                    // A known immediate win dominates every sampled estimate.
                    if index == 0 && next.winner() == Some(root_player) {
                        result.action = Some(action);
                        result.iterations += 1;
                        result.nodes = tree.len() + 1;
                        result.max_tree_depth = result.max_tree_depth.max(1);
                        return result;
                    }
                    let child = tree.len();
                    tree.push(Node::new(next, Some(action)));
                    tree[index].children.push(child);
                    index = child;
                    path.push(index);
                    break;
                }
            }
            if tree[index].children.is_empty() {
                break;
            }
            index = select_child(&tree, index);
            path.push(index);
        }
        result.max_tree_depth = result.max_tree_depth.max(path.len() - 1);
        let mut rollout = tree[index].game.clone();
        for _ in 0..config.rollout_depth {
            if rollout.is_over() {
                break;
            }
            let mut actions = ordered_actions(&rollout, &mut rng);
            let mut advanced = false;
            while !actions.is_empty() {
                // Mostly prefer tactical actions; keep random exploration so
                // quiet moves and alternative cards/abilities remain possible.
                let pick = if rng.below(4) == 0 {
                    rng.below(actions.len() as u64)
                } else {
                    actions.len() - 1
                };
                let action = actions.swap_remove(pick);
                if rollout.apply(action).is_ok() {
                    result.rollout_actions += 1;
                    advanced = true;
                    break;
                }
                result.failed_actions += 1;
            }
            if !advanced {
                break;
            }
        }
        let rewards = rewards(&rollout, &baseline);
        for &node in &path {
            tree[node].visits += 1;
            for (total, reward) in tree[node].rewards.iter_mut().zip(rewards) {
                *total += reward;
            }
        }
        result.iterations += 1;
    }
    let root = &tree[0];
    // Robust child: most visits, then best mean for the root actor, then stable
    // expansion order. UCT itself uses each node's actual acting seat (FFA).
    let mut best = None;
    for &child in &root.children {
        if best.is_none_or(|old: usize| {
            tree[child].visits > tree[old].visits
                || (tree[child].visits == tree[old].visits
                    && mean(&tree[child], root.actor) > mean(&tree[old], root.actor))
        }) {
            best = Some(child);
        }
    }
    result.action = best.and_then(|i| tree[i].action.clone());
    result.nodes = tree.len();
    result
}

fn mean(node: &Node, actor: usize) -> f64 {
    node.rewards[actor] / f64::from(node.visits.max(1))
}
fn select_child(tree: &[Node], parent: usize) -> usize {
    let node = &tree[parent];
    let exploration = 2.0 * f64::from(node.visits.max(1)).ln();
    let mut best = node.children[0];
    let mut best_score = f64::NEG_INFINITY;
    for &child in &node.children {
        let score = mean(&tree[child], node.actor)
            + (exploration / f64::from(tree[child].visits.max(1))).sqrt();
        if score > best_score {
            best = child;
            best_score = score;
        }
    }
    best
}

fn rewards(game: &Game, baseline: &[i64]) -> [f64; 4] {
    let mut rewards = [0.0; 4];
    let scores = heuristic::evaluate_all(game);
    for (seat, player) in game.players().iter().enumerate() {
        rewards[seat] = if let Some(winner) = game.winner() {
            f64::from(u8::from(winner == player.id))
        } else if game.is_eliminated(player.id) {
            0.0
        } else if game.is_draw() {
            0.5
        } else {
            // Relative change avoids saturating every FFA seat with the sum of
            // three opposing armies. Terminal wins/losses stay outside this range.
            let delta = (scores[seat] - baseline[seat]) as f64;
            0.5 + 0.49 * delta / (1000.0 + delta.abs())
        };
    }
    rewards
}

fn ordered_actions(game: &Game, rng: &mut MatchRng) -> Vec<ActionIntent> {
    let mut actions = game.ai_legal_actions();
    let pass = ActionIntent::EndTurn {
        player: game.turn().player,
    };
    if game.pending_promotion().is_none()
        && !actions.contains(&pass)
        && game.validate(&pass).is_ok()
    {
        actions.push(pass);
    }
    for i in (1..actions.len()).rev() {
        let j = rng.below((i + 1) as u64);
        actions.swap(i, j);
    }
    // Stable sorting preserves seeded random order among equal priorities.
    actions.sort_by_key(|action| priority(game, action));
    actions
}
fn priority(game: &Game, action: &ActionIntent) -> i64 {
    match action {
        ActionIntent::Piece {
            kind: ActionKind::Take | ActionKind::Attack,
            target,
            ..
        } => game.at(*target).map_or(500, |p| {
            if game.has_tag(p.id, Tag::Royal) {
                100_000
            } else {
                1000 + match p.archetype.as_str() {
                    "queen" => 900,
                    "rook" => 500,
                    "bishop" | "knight" => 320,
                    "pawn" => 100,
                    _ => 250,
                }
            }
        }),
        ActionIntent::ChoosePromotion { archetype, .. } => match archetype.as_str() {
            "queen" => 900,
            "rook" => 500,
            _ => 300,
        },
        ActionIntent::EndTurn { .. } => -1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cchess_core::Registry;

    #[test]
    fn uct_uses_acting_seats_reward_not_root_or_alternating_sign() {
        let game = Game::four_player(Registry::built_in().unwrap()).unwrap();
        let mut tree = vec![
            Node::new(game.simulation(), None),
            Node::new(game.simulation(), None),
            Node::new(game.simulation(), None),
        ];
        tree[0].children = vec![1, 2];
        tree[0].visits = 20;
        tree[1].visits = 10;
        tree[2].visits = 10;
        tree[1].rewards = [9.0, 1.0, 2.0, 8.0];
        tree[2].rewards = [1.0, 9.0, 8.0, 2.0];
        for (actor, expected) in [(0, 1), (1, 2), (2, 2), (3, 1), (0, 1)] {
            tree[0].actor = actor;
            assert_eq!(select_child(&tree, 0), expected);
        }
    }
}
