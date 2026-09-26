mod support;
use cchess_engine::{
    ai::{
        mcts::{search, SearchConfig},
        AiPolicy,
    },
    cards::{CardChoice, CardMatcher},
    clock::ClockUnit,
    content::EffectDef,
    ActionIntent, Game, GameAi, Registry, RuleError,
};
use support::*;

#[test]
fn simulation_rolls_back_effects_and_counter_when_resolution_fails() {
    let reg = registry(|_, cards| {
        let card = cards.iter_mut().find(|c| c.id == "more_mana").unwrap();
        card.triggers[0].event = CardMatcher::Clock(ClockUnit::Movements);
        card.triggers[0].effects = vec![EffectDef::Sequence(vec![EffectDef::AddMana(1); 1024])];
    });
    let mut live = setup(reg, &[], &["more_mana", "mana"], &["mana"], 2, 0);
    play(&mut live, "more_mana", CardChoice::None);
    let mut simulation = live.simulation();
    let before = simulation.clone();
    let action = ActionIntent::PlayCard {
        player: 0,
        card: hand(&live, "mana"),
        target: CardChoice::None,
    };
    assert_eq!(
        simulation.apply(action.clone()),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(simulation, before);
    assert_eq!(live.apply(action), Err(RuleError::ResolutionLimit));
    assert_eq!(simulation.action_count(), live.action_count());
}

#[test]
fn seeded_policies_use_committed_count_on_history_free_branches() {
    let mut live = Game::classical(Registry::built_in().unwrap())
        .unwrap()
        .with_ai(vec![(0, AiPolicy::Random), (1, AiPolicy::Random)], 17)
        .unwrap();
    for _ in 0..12 {
        live.play_ai_step().unwrap().unwrap();
    }
    let mut simulation = live.simulation();
    assert!(simulation.log().is_empty());
    assert_eq!(simulation.action_count(), 12);
    let config = SearchConfig {
        iterations: 16,
        rollout_depth: 2,
    };
    assert_eq!(
        search(&live, config).action,
        search(&simulation, config).action
    );
    for _ in 0..24 {
        let Some(action) = live.choose_ai_action() else {
            break;
        };
        assert_eq!(simulation.choose_ai_action(), Some(action.clone()));
        assert_eq!(simulation.apply(action.clone()), live.apply(action));
        assert_eq!(simulation.action_count(), live.action_count());
    }
}
