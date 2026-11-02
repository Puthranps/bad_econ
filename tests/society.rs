#![cfg(feature = "compute")]
mod support;

use anyhow::Result;
use bad_econ::{
    domain::society::*,
    simulation::society::{SocietyReport, SocietySimulation},
};
use std::collections::BTreeSet;

fn adult(slot: u32, strategy: Strategy, trade_behavior: TradeBehavior) -> AgentSeed {
    AgentSeed {
        slot,
        strategy,
        trade_behavior,
        age: 20,
    }
}

#[test]
fn gpu_society_replays_across_workgroups_and_rejects_unbounded_inputs() -> Result<()> {
    let scenario = SocietyScenario::mixed(19, 42)?;
    let first = SocietyReport::run(scenario.clone(), support::device(32)?, 24)?;
    let second = SocietyReport::run(scenario, support::device(64)?, 24)?;
    assert_eq!(first.rounds, second.rounds);
    let invalid = SocietyConfig {
        project_effort: u32::MAX,
        work_effort: u32::MAX,
        ..SocietyConfig::default()
    };
    assert!(SocietyScenario::new(invalid.clone()).is_err());
    assert!(serde_json::from_value::<SocietyScenario>(serde_json::to_value(invalid)?).is_err());
    assert!(SocietyScenario::mixed(u32::MAX, 42).is_err());
    Ok(())
}

#[test]
fn gpu_starvation_removes_agents_and_transfers_cash_estates() -> Result<()> {
    let scenario = SocietyScenario::new(SocietyConfig {
        agents: vec![
            adult(0, Strategy::FreeRider, TradeBehavior::Fair),
            adult(1, Strategy::FreeRider, TradeBehavior::Fair),
        ],
        capacity: 2,
        neighborhood_size: 2,
        initial_food: 0,
        initial_cash: 7,
        daily_effort: 1,
        project_effort: 1,
        project_productivity: 1,
        work_effort: 0,
        work_food: 0,
        jobs: 0,
        firm_food: 0,
        firm_cash: 0,
        daily_food_need: 2,
        child_food_need: 1,
        starvation_days: 2,
        birth_rule: BirthRule::Disabled,
        ..SocietyConfig::default()
    })?;
    let report = SocietyReport::run(scenario, support::device(64)?, 3)?;
    assert_eq!(report.rounds[0].summary.population, 2);
    assert_eq!(report.rounds[0].summary.consumed_food, 2);
    assert_eq!(report.rounds[1].summary.deaths, 2);
    assert_eq!(report.rounds[1].summary.population, 0);
    assert!(
        report.rounds[1]
            .agents
            .iter()
            .all(|agent| agent.life == Life::Dead)
    );
    assert!(
        report.rounds[2]
            .agents
            .iter()
            .all(|agent| agent.life == Life::Vacant)
    );
    assert_eq!(report.rounds[2].firms[0].cash, 14);
    assert!(report.rounds.iter().all(|round| round.summary.cash == 14));
    Ok(())
}

fn trade_scenario(trade_rule: TradeRule) -> Result<SocietyScenario> {
    SocietyScenario::new(SocietyConfig {
        agents: vec![
            adult(0, Strategy::FreeRider, TradeBehavior::Fair),
            adult(1, Strategy::Cooperator, TradeBehavior::Cheat),
        ],
        capacity: 2,
        neighborhood_size: 2,
        initial_food: 0,
        initial_cash: 10,
        daily_effort: 2,
        project_effort: 2,
        project_productivity: 4,
        work_effort: 0,
        jobs: 0,
        firm_cash: 0,
        firm_food: 0,
        daily_food_need: 3,
        child_food_need: 1,
        food_price: 2,
        sharing: SharingRule::ContributorsOnly,
        birth_rule: BirthRule::Disabled,
        trade_rule,
        ..SocietyConfig::default()
    })
}

#[test]
fn gpu_trade_fraud_is_a_real_transfer_and_memory_refuses_the_offender() -> Result<()> {
    let report = SocietyReport::run(trade_scenario(TradeRule::Trust)?, support::device(64)?, 2)?;
    let buyer = &report.rounds[0].agents[0];
    let seller = &report.rounds[0].agents[1];
    assert_eq!(buyer.cash, 8);
    assert_eq!(buyer.bought_food, 0);
    assert_eq!(buyer.cheated_trades, 1);
    assert_eq!(buyer.last_fraud_identity, seller.identity);
    assert_eq!(seller.cash, 12);
    assert_eq!(seller.frauds_committed, 1);
    assert_eq!(seller.sold_food, 0);
    let next = &report.rounds[1].agents[0];
    assert_eq!(next.cash, 8);
    assert_eq!(next.cheated_trades, 0);
    assert_eq!(next.refused_trades, 1);
    assert!(report.rounds.iter().all(|round| round.summary.cash == 20));
    let escrow = SocietyReport::run(trade_scenario(TradeRule::Escrow)?, support::device(64)?, 1)?;
    assert_eq!(escrow.rounds[0].agents[0].cash, 8);
    assert_eq!(escrow.rounds[0].agents[0].bought_food, 1);
    assert_eq!(escrow.rounds[0].agents[0].food_shortfall, 0);
    assert_eq!(escrow.rounds[0].summary.frauds, 0);
    Ok(())
}

#[test]
fn gpu_parent_funded_births_grow_population_without_child_labor() -> Result<()> {
    let scenario = SocietyScenario::new(SocietyConfig {
        agents: vec![adult(0, Strategy::Cooperator, TradeBehavior::Fair)],
        capacity: 4,
        neighborhood_size: 4,
        initial_food: 100,
        daily_effort: 4,
        project_effort: 1,
        project_productivity: 2,
        daily_food_need: 2,
        child_food_need: 1,
        jobs: 1,
        work_effort: 1,
        wage_offer: 1,
        maximum_wage: 1,
        reservation_wage: 1,
        adult_age: 3,
        birth_interval: 1,
        birth_food: 4,
        ..SocietyConfig::default()
    })?;
    let report = SocietyReport::run(scenario, support::device(64)?, 10)?;
    assert_eq!(report.rounds[0].summary.births, 1);
    assert!(
        report
            .rounds
            .iter()
            .any(|round| round.summary.population > 1)
    );
    assert!(
        report
            .rounds
            .iter()
            .flat_map(|round| &round.agents)
            .filter(|agent| agent.life == Life::Child)
            .all(|agent| !agent.employed)
    );
    assert!(
        report
            .rounds
            .iter()
            .flat_map(|round| &round.agents)
            .any(|agent| agent.parent_identity == 1 && agent.life == Life::Adult && agent.employed)
    );
    for round in &report.rounds {
        let living = round
            .agents
            .iter()
            .filter(|agent| matches!(agent.life, Life::Adult | Life::Child))
            .collect::<Vec<_>>();
        assert_eq!(
            living.len(),
            living
                .iter()
                .map(|agent| agent.identity)
                .collect::<BTreeSet<_>>()
                .len()
        );
    }
    Ok(())
}

#[test]
fn gpu_new_id_reuses_a_dead_slot_and_matures_into_a_replacement_worker() -> Result<()> {
    let scenario = SocietyScenario::new(SocietyConfig {
        agents: vec![
            adult(0, Strategy::FreeRider, TradeBehavior::Fair),
            adult(1, Strategy::Cooperator, TradeBehavior::Fair),
        ],
        capacity: 2,
        neighborhood_size: 2,
        initial_food: 0,
        initial_cash: 0,
        firm_food: 0,
        firm_cash: 100,
        daily_effort: 4,
        project_effort: 2,
        project_productivity: 4,
        work_effort: 0,
        work_food: 0,
        jobs: 1,
        daily_food_need: 6,
        child_food_need: 1,
        sharing: SharingRule::ContributorsOnly,
        starvation_days: 2,
        adult_age: 2,
        birth_interval: 1,
        birth_food: 2,
        wage_offer: 1,
        maximum_wage: 1,
        reservation_wage: 1,
        food_price: 1000,
        ..SocietyConfig::default()
    })?;
    let report = SocietyReport::run(scenario, support::device(64)?, 8)?;
    assert!(report.rounds[0].agents[0].employed);
    assert_eq!(report.rounds[1].agents[0].life, Life::Dead);
    assert_eq!(report.rounds[2].agents[0].life, Life::Vacant);
    let newborn = &report.rounds[3].agents[0];
    assert_eq!(newborn.life, Life::Child);
    assert_ne!(newborn.identity, 1);
    assert_eq!(newborn.parent_identity, 2);
    assert!(
        report
            .rounds
            .iter()
            .any(|round| round.agents[0].identity == newborn.identity
                && round.agents[0].life == Life::Adult
                && round.agents[0].employed)
    );
    assert!(report.rounds.iter().all(|round| round.summary.cash == 100));
    Ok(())
}

#[test]
fn gpu_wage_counteroffer_changes_hiring_without_employing_children() -> Result<()> {
    let config = SocietyConfig {
        agents: vec![
            adult(0, Strategy::Cooperator, TradeBehavior::Fair),
            AgentSeed {
                slot: 1,
                age: 0,
                strategy: Strategy::Cooperator,
                trade_behavior: TradeBehavior::Fair,
            },
        ],
        capacity: 2,
        neighborhood_size: 2,
        jobs: 1,
        wage_offer: 2,
        maximum_wage: 5,
        reservation_wage: 4,
        birth_rule: BirthRule::Disabled,
        ..SocietyConfig::default()
    };
    let mut negotiated =
        SocietySimulation::new(SocietyScenario::new(config.clone())?, support::device(64)?)?;
    let round = negotiated.advance()?;
    assert_eq!(round.agents[0].wage, 4);
    assert_eq!(round.agents[0].reservation_wage, 4);
    assert!(!round.agents[1].employed);
    let mut posted = SocietySimulation::new(
        SocietyScenario::new(SocietyConfig {
            wage_rule: WageRule::Posted,
            ..config
        })?,
        support::device(64)?,
    )?;
    assert_eq!(posted.advance()?.summary.employed, 0);
    Ok(())
}
