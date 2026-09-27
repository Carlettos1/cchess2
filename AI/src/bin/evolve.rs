use cchess_ai::{
    evolution::{choose_action, mutate, update_ratings, ActionTally, Genome, Stamp},
    heuristic,
};
use cchess_core::{Game, Registry};
use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut output = PathBuf::from("ai-stamps");
    let mut generations = 3_u32;
    let mut population = Some(8_usize);
    let mut max_actions = 300_usize;
    let mut seed = 42_u64;
    let mut full_board = false;
    let mut resume = None;
    let mut threads = std::thread::available_parallelism()?.get();
    let mut games_per_thread = 1_usize;
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).ok_or("each option requires a value")?;
        match args[i].as_str() {
            "--output" => output = value.into(),
            "--generations" => generations = value.parse()?,
            "--population" => {
                population = if value == "auto" {
                    None
                } else {
                    Some(value.parse()?)
                }
            }
            "--max-actions" => max_actions = value.parse()?,
            "--seed" => seed = value.parse()?,
            "--preset" => match value.as_str() {
                "classical" => full_board = false,
                "full-board" => full_board = true,
                _ => return Err("preset must be classical or full-board".into()),
            },
            "--resume" => resume = Some(PathBuf::from(value)),
            "--threads" => {
                threads = if value == "auto" {
                    std::thread::available_parallelism()?.get()
                } else {
                    value.parse()?
                }
            }
            "--games-per-thread" => games_per_thread = value.parse()?,
            _ => return Err(format!("unknown option {}", args[i]).into()),
        }
        i += 2;
    }
    if generations == 0 || max_actions == 0 || threads == 0 || games_per_thread == 0 {
        return Err(
            "generations, max-actions, threads and games-per-thread must be positive".into(),
        );
    }
    let capacity = threads.saturating_mul(games_per_thread);
    let requested_population = population.unwrap_or_else(|| population_for_games(capacity));
    if requested_population < 2 {
        return Err("match capacity is too small; at least two games are required".into());
    }
    fs::create_dir_all(&output)?;
    let (mut stamps, first_generation) = if let Some(path) = resume {
        let previous: Vec<Stamp> = ron::from_str(&fs::read_to_string(&path)?)?;
        if previous.len() < 2 {
            return Err("resume generation must contain at least two stamps".into());
        }
        let population = previous.len();
        let next_generation = previous
            .iter()
            .map(|stamp| stamp.generation)
            .max()
            .unwrap_or(0)
            + 1;
        (
            breed(previous, population, next_generation, seed),
            next_generation,
        )
    } else {
        let founder = Stamp::founder("g0-000", Genome::default());
        let mut initial = vec![founder.clone()];
        for n in 1..requested_population {
            initial.push(Stamp::founder(
                format!("g0-{n:03}"),
                mutate(&founder.genome, mix(seed, n as u64)),
            ));
        }
        (initial, 0)
    };
    let population = stamps.len();
    let games = population * (population - 1);
    println!(
        "training {population} agents with {games} games/generation on {} worker thread(s)",
        threads.min(games)
    );
    for offset in 0..generations {
        let generation = first_generation + offset;
        let started = std::time::Instant::now();
        let results = play_generation(&stamps, max_actions, full_board, threads)?;
        let mut tallies = vec![ActionTally::default(); stamps.len()];
        for result in results {
            merge(&mut tallies[result.a], result.tally_a);
            merge(&mut tallies[result.b], result.tally_b);
            let (left, right) = stamps.split_at_mut(result.b);
            update_ratings(&mut left[result.a], &mut right[0], result.score);
        }
        for (stamp, tally) in stamps.iter_mut().zip(tallies) {
            stamp.personality = tally.personality();
        }
        stamps.sort_by(|a, b| b.rating.cmp(&a.rating).then_with(|| a.id.cmp(&b.id)));
        let path = output.join(format!("generation-{generation:03}.ron"));
        fs::write(
            &path,
            ron::ser::to_string_pretty(&stamps, ron::ser::PrettyConfig::default())?,
        )?;
        println!(
            "generation {generation}: {} rating {} in {:.2?} -> {}",
            stamps[0].id,
            stamps[0].rating,
            started.elapsed(),
            path.display()
        );
        if offset + 1 < generations {
            stamps = breed(stamps, population, generation + 1, seed);
        }
    }
    Ok(())
}

fn play(
    a: &Genome,
    b: &Genome,
    swap: bool,
    max_actions: usize,
    full_board: bool,
) -> Result<(i32, ActionTally, ActionTally), String> {
    let registry = Registry::built_in().map_err(|e| format!("{e:?}"))?;
    let mut game = if full_board {
        Game::full_board(registry, false)?
    } else {
        Game::classical(registry)?
    };
    let ids = [game.players()[0].id, game.players()[1].id];
    let mut tallies = [ActionTally::default(), ActionTally::default()];
    for _ in 0..max_actions {
        if game.is_over() {
            break;
        }
        let seat = usize::from(game.turn().player == ids[1]);
        let genome = if (seat == 0) ^ swap { a } else { b };
        let Some(action) = choose_action(&game, genome) else {
            break;
        };
        tallies[usize::from((seat == 1) ^ swap)].observe(&action);
        game.apply(action).map_err(|e| e.to_string())?;
    }
    let score = match game.winner() {
        Some(winner) if (winner == ids[0]) ^ swap => 1000,
        Some(_) => 0,
        None => {
            // A capped match is adjudicated by the same public position evaluator
            // used by the policies. Exact ties remain draws.
            let sa = heuristic::evaluate(&game, ids[usize::from(swap)]);
            let sb = heuristic::evaluate(&game, ids[usize::from(!swap)]);
            match sa.cmp(&sb) {
                std::cmp::Ordering::Greater => 1000,
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Equal => 500,
            }
        }
    };
    Ok((score, tallies[0], tallies[1]))
}

#[derive(Clone, Copy)]
struct MatchJob {
    a: usize,
    b: usize,
    swap: bool,
}

struct MatchResult {
    a: usize,
    b: usize,
    score: i32,
    tally_a: ActionTally,
    tally_b: ActionTally,
}

fn play_generation(
    stamps: &[Stamp],
    max_actions: usize,
    full_board: bool,
    threads: usize,
) -> Result<Vec<MatchResult>, String> {
    let mut jobs = Vec::with_capacity(stamps.len() * (stamps.len() - 1));
    for a in 0..stamps.len() {
        for b in (a + 1)..stamps.len() {
            for swap in [false, true] {
                jobs.push(MatchJob { a, b, swap });
            }
        }
    }
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<MatchResult, String>>>> =
        Mutex::new((0..jobs.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..threads.min(jobs.len()) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(index).copied() else {
                    break;
                };
                let outcome = play(
                    &stamps[job.a].genome,
                    &stamps[job.b].genome,
                    job.swap,
                    max_actions,
                    full_board,
                )
                .map(|(score, tally_a, tally_b)| MatchResult {
                    a: job.a,
                    b: job.b,
                    score,
                    tally_a,
                    tally_b,
                });
                results.lock().unwrap()[index] = Some(outcome);
            });
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|result| result.expect("every queued match produces a result"))
        .collect()
}

fn population_for_games(capacity: usize) -> usize {
    let mut population = 1_usize;
    while population
        .checked_add(1)
        .and_then(|next| next.checked_mul(next - 1))
        .is_some_and(|games| games <= capacity)
    {
        population += 1;
    }
    population
}

fn breed(mut stamps: Vec<Stamp>, population: usize, generation: u32, seed: u64) -> Vec<Stamp> {
    stamps.sort_by(|a, b| b.rating.cmp(&a.rating).then_with(|| a.id.cmp(&b.id)));
    let elite_count = (population / 4).max(1);
    let elites: Vec<_> = stamps.into_iter().take(elite_count).collect();
    let mut next = elites.clone();
    while next.len() < population {
        let parent = &elites[(next.len() - elite_count) % elites.len()];
        next.push(parent.child(
            format!("g{generation}-{:03}", next.len()),
            mix(seed ^ u64::from(generation), next.len() as u64),
        ));
    }
    next
}

fn merge(a: &mut ActionTally, b: ActionTally) {
    a.total += b.total;
    a.aggressive += b.aggressive;
    a.moves += b.moves;
    a.creative += b.creative;
    a.passes += b.passes;
}
fn mix(seed: u64, n: u64) -> u64 {
    seed ^ n.wrapping_mul(0x9e3779b97f4a7c15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_population_uses_color_swapped_match_capacity() {
        assert_eq!(population_for_games(16), 4);
        assert_eq!(population_for_games(64), 8);
        assert_eq!(population_for_games(240), 16);
    }

    #[test]
    fn parallel_results_keep_canonical_job_order() {
        let base = Stamp::founder("a", Genome::default());
        let stamps = vec![base.clone(), base.child("b", 1), base.child("c", 2)];
        let serial = play_generation(&stamps, 1, false, 1).unwrap();
        let parallel = play_generation(&stamps, 1, false, 4).unwrap();
        assert_eq!(serial.len(), 6);
        assert!(serial.iter().zip(parallel).all(|(a, b)| (
            a.a,
            a.b,
            a.score,
            a.tally_a.total,
            a.tally_b.total
        ) == (
            b.a,
            b.b,
            b.score,
            b.tally_a.total,
            b.tally_b.total
        )));
    }
}
