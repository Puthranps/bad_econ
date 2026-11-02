#![cfg(feature = "compute")]
mod support;

use anyhow::Result;
use bad_econ::{domain::life::*, simulation::life::LifeSimulation};
use std::collections::{BTreeMap, BTreeSet};

fn empty() -> LifeConfig {
    LifeConfig {
        width: 8,
        height: 8,
        density_percent: 0,
        initial_food: 0,
        initial_energy: 20,
        harvest: 1,
        metabolism: 1,
        maturity_age: 0,
        maximum_age: 100,
        birth_threshold: 100,
        child_energy: 5,
        rules: LifeRules {
            regrowth: 0,
            cooperation: 2,
            exploitation: 0,
            mutation_per_mille: 0,
        },
        ..LifeConfig::default()
    }
}

fn place(simulation: &mut LifeSimulation, x: u32, y: u32, brush: Brush) -> Result<()> {
    simulation.paint(BrushStroke {
        x,
        y,
        radius: 0,
        brush,
    })
}

#[test]
fn gpu_life_shares_across_toroidal_seams_but_not_distant_cells() -> Result<()> {
    let mut simulation = LifeSimulation::new(LifeScenario::new(empty())?, support::device(64)?)?;
    place(&mut simulation, 0, 0, Brush::Cooperator)?;
    place(&mut simulation, 7, 7, Brush::Cooperator)?;
    place(&mut simulation, 4, 4, Brush::Cooperator)?;
    let frame = simulation.advance()?;
    assert_eq!(frame.cells[0].energy, 21);
    assert_eq!(frame.cells[63].energy, 21);
    assert_eq!(frame.cells[36].energy, 19);
    assert_eq!(frame.summary.energy, 61);
    assert_eq!(frame.summary.population, 3);
    Ok(())
}

#[test]
fn gpu_life_exploitation_transfers_energy_from_the_neighbor() -> Result<()> {
    let mut config = empty();
    config.rules.exploitation = 2;
    let mut simulation = LifeSimulation::new(LifeScenario::new(config)?, support::device(64)?)?;
    place(&mut simulation, 0, 0, Brush::Cooperator)?;
    place(&mut simulation, 7, 7, Brush::Exploiter)?;
    let frame = simulation.advance()?;
    assert_eq!(frame.cells[0].energy, 15);
    assert_eq!(frame.cells[63].energy, 24);
    assert_eq!(frame.summary.energy, 39);
    assert_eq!(frame.cells[0].action, CellAction::Cooperate);
    assert_eq!(frame.cells[63].action, CellAction::Exploit);
    Ok(())
}

#[test]
fn gpu_life_birth_collision_has_one_winner_and_transfers_energy_once() -> Result<()> {
    let mut config = empty();
    config.width = 4;
    config.height = 4;
    config.birth_threshold = 10;
    config.rules.cooperation = 0;
    let mut simulation = LifeSimulation::new(LifeScenario::new(config)?, support::device(64)?)?;
    simulation.paint(BrushStroke {
        x: 0,
        y: 0,
        radius: 16,
        brush: Brush::Cooperator,
    })?;
    place(&mut simulation, 0, 0, Brush::Erase)?;
    let before = simulation.snapshot();
    let after = simulation.advance()?;
    assert_eq!(after.summary.births, 1);
    assert_eq!(after.summary.population, 16);
    assert_eq!(after.summary.energy, before.summary.energy - 15);
    assert_eq!(after.cells[0].event, CellEvent::Born);
    assert_eq!(after.cells[0].energy, 5);
    let parent_id = after.cells[0].parent_identity;
    let parent = after
        .cells
        .iter()
        .find(|cell| cell.identity == parent_id)
        .expect("winning parent");
    assert_eq!(parent.energy, 14);
    assert_eq!(
        after.cells.iter().filter(|cell| cell.energy == 19).count(),
        14
    );
    assert_eq!(
        after
            .cells
            .iter()
            .map(|cell| cell.identity)
            .collect::<BTreeSet<_>>()
            .len(),
        16
    );
    assert!(
        !before
            .cells
            .iter()
            .any(|cell| cell.identity == after.cells[0].identity)
    );
    Ok(())
}

#[test]
fn gpu_life_starvation_age_and_invalid_controls_are_explicit() -> Result<()> {
    let config = LifeConfig {
        initial_energy: 1,
        metabolism: 2,
        ..empty()
    };
    let scenario = LifeScenario::new(config.clone())?;
    let mut simulation = LifeSimulation::new(scenario.clone(), support::device(64)?)?;
    place(&mut simulation, 0, 0, Brush::Cooperator)?;
    let before = simulation.snapshot();
    assert!(
        simulation
            .set_rules(LifeRules {
                regrowth: 9,
                ..config.rules
            })
            .is_err()
    );
    assert!(
        simulation
            .paint(BrushStroke {
                x: 8,
                y: 0,
                radius: 0,
                brush: Brush::Food
            })
            .is_err()
    );
    assert!(
        simulation
            .paint(BrushStroke {
                x: 0,
                y: 0,
                radius: 17,
                brush: Brush::Food
            })
            .is_err()
    );
    assert_eq!(simulation.snapshot(), before);
    let dead = simulation.advance()?;
    assert_eq!(dead.summary.deaths, 1);
    assert_eq!(dead.summary.hungry, 1);
    assert_eq!(dead.summary.population, 0);
    assert_eq!(dead.cells[0].life, CellLife::Dead);
    assert_eq!(simulation.advance()?.cells[0].life, CellLife::Vacant);
    simulation.reset(scenario)?;
    assert_eq!(simulation.snapshot().tick, 0);
    assert_eq!(simulation.snapshot().summary.population, 0);
    let aged = LifeScenario::new(LifeConfig {
        maturity_age: 0,
        maximum_age: 1,
        ..empty()
    })?;
    simulation.reset(aged)?;
    place(&mut simulation, 0, 0, Brush::Cooperator)?;
    let frame = simulation.advance()?;
    assert_eq!(frame.cells[0].life, CellLife::Dead);
    assert_eq!(frame.summary.hungry, 0);
    assert_eq!(frame.summary.energy, 0);
    assert!(
        LifeScenario::new(LifeConfig {
            cooperator_percent: u32::MAX,
            exploiter_percent: u32::MAX,
            ..config
        })
        .is_err()
    );
    Ok(())
}

#[test]
fn gpu_life_replays_births_mutations_and_lineage_across_workgroups() -> Result<()> {
    let scenario = LifeScenario::new(LifeConfig {
        width: 16,
        height: 12,
        density_percent: 55,
        initial_energy: 24,
        maturity_age: 0,
        maximum_age: 25,
        birth_threshold: 12,
        child_energy: 5,
        rules: LifeRules {
            mutation_per_mille: 100,
            ..LifeRules::default()
        },
        ..LifeConfig::default()
    })?;
    let mut first = LifeSimulation::new(scenario.clone(), support::device(32)?)?;
    let mut seen = first
        .snapshot()
        .cells
        .iter()
        .filter(|cell| cell.life == CellLife::Alive)
        .map(|cell| (cell.identity, cell.strategy))
        .collect::<BTreeMap<_, _>>();
    let mut frames = Vec::new();
    let mut mutations = 0;
    for _ in 0..40 {
        let frame = first.advance()?;
        let identities = frame
            .cells
            .iter()
            .filter(|cell| cell.life == CellLife::Alive)
            .map(|cell| cell.identity)
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), frame.summary.population as usize);
        for child in frame
            .cells
            .iter()
            .filter(|cell| cell.event == CellEvent::Born)
        {
            let parent = seen
                .get(&child.parent_identity)
                .expect("known parent identity");
            mutations += usize::from(*parent != child.strategy);
            assert!(seen.insert(child.identity, child.strategy).is_none());
        }
        frames.push(frame);
    }
    assert!(frames.iter().any(|frame| frame.summary.births > 0));
    assert!(mutations > 0);
    let mut second = LifeSimulation::new(scenario, support::device(64)?)?;
    for expected in frames {
        assert_eq!(second.advance()?, expected);
    }
    Ok(())
}
