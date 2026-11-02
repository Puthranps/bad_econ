use crate::arithmetic::DrawKey;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_CAPACITY: u32 = 4096;
pub const MAX_DAYS: u32 = 365;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    Cooperator,
    FreeRider,
    Reciprocator,
    Imitator,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeBehavior {
    Fair,
    Cheat,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SharingRule {
    Everyone,
    ContributorsOnly,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeRule {
    Trust,
    Escrow,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WageRule {
    Negotiate,
    Posted,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BirthRule {
    Disabled,
    Surplus,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Life {
    Vacant,
    Child,
    Adult,
    Dead,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Rest,
    FreeRide,
    Contribute,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSeed {
    pub slot: u32,
    pub strategy: Strategy,
    pub trade_behavior: TradeBehavior,
    pub age: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SocietyConfig {
    pub agents: Vec<AgentSeed>,
    pub capacity: u32,
    pub neighborhood_size: u32,
    pub initial_food: u32,
    pub initial_cash: u32,
    pub daily_effort: u32,
    pub project_effort: u32,
    pub project_productivity: u32,
    pub daily_food_need: u32,
    pub child_food_need: u32,
    pub reciprocity_percent: u32,
    pub sharing: SharingRule,
    pub trade_rule: TradeRule,
    pub food_price: u32,
    pub firm_cash: u32,
    pub firm_food: u32,
    pub jobs: u32,
    pub work_effort: u32,
    pub work_food: u32,
    pub wage_offer: u32,
    pub maximum_wage: u32,
    pub reservation_wage: u32,
    pub wage_rule: WageRule,
    pub adult_age: u32,
    pub starvation_days: u32,
    pub birth_rule: BirthRule,
    pub birth_food: u32,
    pub birth_interval: u32,
}

impl Default for SocietyConfig {
    fn default() -> Self {
        Self {
            agents: (0..8)
                .map(|slot| AgentSeed {
                    slot: slot * 2,
                    strategy: Strategy::Cooperator,
                    trade_behavior: TradeBehavior::Fair,
                    age: 20,
                })
                .collect(),
            capacity: 16,
            neighborhood_size: 16,
            initial_food: 24,
            initial_cash: 20,
            daily_effort: 8,
            project_effort: 3,
            project_productivity: 3,
            daily_food_need: 10,
            child_food_need: 2,
            reciprocity_percent: 50,
            sharing: SharingRule::Everyone,
            trade_rule: TradeRule::Trust,
            food_price: 2,
            firm_cash: 200,
            firm_food: 32,
            jobs: 2,
            work_effort: 3,
            work_food: 12,
            wage_offer: 4,
            maximum_wage: 8,
            reservation_wage: 4,
            wage_rule: WageRule::Negotiate,
            adult_age: 14,
            starvation_days: 5,
            birth_rule: BirthRule::Surplus,
            birth_food: 20,
            birth_interval: 20,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "SocietyConfig", into = "SocietyConfig")]
pub struct SocietyScenario(SocietyConfig);

impl TryFrom<SocietyConfig> for SocietyScenario {
    type Error = anyhow::Error;
    fn try_from(config: SocietyConfig) -> Result<Self> {
        Self::new(config)
    }
}

impl From<SocietyScenario> for SocietyConfig {
    fn from(scenario: SocietyScenario) -> Self {
        scenario.0
    }
}

impl SocietyScenario {
    pub fn new(config: SocietyConfig) -> Result<Self> {
        let bounded = [
            config.initial_food,
            config.initial_cash,
            config.firm_cash,
            config.firm_food,
            config.birth_food,
        ];
        let small = [
            config.daily_effort,
            config.project_effort,
            config.work_effort,
            config.work_food,
            config.wage_offer,
            config.maximum_wage,
            config.reservation_wage,
        ];
        let slots = config
            .agents
            .iter()
            .map(|agent| agent.slot)
            .collect::<BTreeSet<_>>();
        let bounds = [
            (1..=MAX_CAPACITY).contains(&config.capacity),
            (2..=32).contains(&config.neighborhood_size),
            !config.agents.is_empty(),
            slots.len() == config.agents.len(),
            config
                .agents
                .iter()
                .all(|agent| agent.slot < config.capacity && agent.age <= 1000),
            bounded.into_iter().all(|value| value <= 1_000_000),
            small.into_iter().all(|value| value <= 1000),
            config.project_effort > 0,
            config
                .project_effort
                .checked_add(config.work_effort)
                .is_some_and(|effort| effort <= config.daily_effort),
            (1..=16).contains(&config.project_productivity),
            (1..=32).contains(&config.daily_food_need),
            (1..=config.daily_food_need).contains(&config.child_food_need),
            config.reciprocity_percent <= 100,
            (1..=1000).contains(&config.food_price),
            config.jobs <= config.neighborhood_size,
            config.wage_offer <= config.maximum_wage,
            (1..=365).contains(&config.adult_age),
            (1..=365).contains(&config.starvation_days),
            (1..=365).contains(&config.birth_interval),
        ];
        match bounds.into_iter().all(std::convert::identity) {
            true => Ok(Self(config)),
            false => Err(anyhow!(
                "society settings exceed population, effort, food, wage or life-cycle bounds"
            )),
        }
    }

    pub fn mixed(agents: u32, seed: u64) -> Result<Self> {
        match (1..=MAX_CAPACITY / 2).contains(&agents) {
            false => Err(anyhow!(
                "society requires 1..={} initial agents",
                MAX_CAPACITY / 2
            )),
            true => Self::new(SocietyConfig {
                capacity: agents * 2,
                agents: (0..agents)
                    .map(|actor| {
                        let draw = |purpose| {
                            DrawKey {
                                seed,
                                day: 0,
                                phase: 0,
                                actor: u64::from(actor),
                                purpose,
                                index: 0,
                            }
                            .draw()
                        };
                        AgentSeed {
                            slot: actor * 2,
                            strategy: match draw(0x534f4300) % 10 {
                                0..=1 => Strategy::Cooperator,
                                2 => Strategy::FreeRider,
                                3..=6 => Strategy::Reciprocator,
                                _ => Strategy::Imitator,
                            },
                            trade_behavior: match draw(0x534f4301) % 5 {
                                0 => TradeBehavior::Cheat,
                                _ => TradeBehavior::Fair,
                            },
                            age: 20,
                        }
                    })
                    .collect(),
                ..SocietyConfig::default()
            }),
        }
    }

    pub fn config(&self) -> &SocietyConfig {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentOutcome {
    pub slot: u32,
    pub identity: u32,
    pub parent_identity: u32,
    pub neighborhood: u32,
    pub strategy: Strategy,
    pub trade_behavior: TradeBehavior,
    pub life: Life,
    pub age: u32,
    pub action: Action,
    pub food: u32,
    pub cash: u32,
    pub contributed_effort: u32,
    pub private_food: u32,
    pub shared_food: u32,
    pub wage: u32,
    pub reservation_wage: u32,
    pub employed: bool,
    pub food_shortfall: u32,
    pub consecutive_shortages: u32,
    pub bought_food: u32,
    pub sold_food: u32,
    pub cheated_trades: u32,
    pub refused_trades: u32,
    pub frauds_committed: u32,
    pub last_fraud_identity: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmOutcome {
    pub neighborhood: u32,
    pub cash: u32,
    pub food: u32,
    pub wage_offer: u32,
    pub workers: u32,
    pub vacancies: u32,
    pub produced_food: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundSummary {
    pub day: u32,
    pub population: u32,
    pub adults: u32,
    pub children: u32,
    pub contributors: u32,
    pub employed: u32,
    pub hungry_agents: u32,
    pub births: u32,
    pub deaths: u32,
    pub trades: u32,
    pub frauds: u32,
    pub food_reserves: u64,
    pub cash: u64,
    pub produced_food: u64,
    pub consumed_food: u64,
    pub abandoned_food: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocietyRound {
    pub summary: RoundSummary,
    pub agents: Vec<AgentOutcome>,
    pub firms: Vec<FirmOutcome>,
}
