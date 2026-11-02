use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LifeRules {
    pub regrowth: u32,
    pub cooperation: u32,
    pub exploitation: u32,
    pub mutation_per_mille: u32,
}

impl Default for LifeRules {
    fn default() -> Self {
        Self {
            regrowth: 2,
            cooperation: 2,
            exploitation: 2,
            mutation_per_mille: 10,
        }
    }
}

impl LifeRules {
    pub fn new(rules: Self) -> Result<Self> {
        match [
            rules.regrowth <= 8,
            rules.cooperation <= 8,
            rules.exploitation <= 8,
            rules.mutation_per_mille <= 100,
        ]
        .into_iter()
        .all(std::convert::identity)
        {
            true => Ok(rules),
            false => Err(anyhow!(
                "regrowth, cooperation and exploitation must be 0..=8; mutation must be 0..=100 per mille"
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LifeConfig {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub density_percent: u32,
    pub cooperator_percent: u32,
    pub exploiter_percent: u32,
    pub rules: LifeRules,
    pub food_capacity: u32,
    pub initial_food: u32,
    pub initial_energy: u32,
    pub harvest: u32,
    pub metabolism: u32,
    pub maturity_age: u32,
    pub maximum_age: u32,
    pub birth_threshold: u32,
    pub child_energy: u32,
}

impl Default for LifeConfig {
    fn default() -> Self {
        Self {
            width: 96,
            height: 64,
            seed: 42,
            density_percent: 35,
            cooperator_percent: 33,
            exploiter_percent: 33,
            rules: LifeRules::default(),
            food_capacity: 32,
            initial_food: 16,
            initial_energy: 18,
            harvest: 6,
            metabolism: 3,
            maturity_age: 12,
            maximum_age: 160,
            birth_threshold: 40,
            child_energy: 12,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "LifeConfig", into = "LifeConfig")]
pub struct LifeScenario(LifeConfig);

impl TryFrom<LifeConfig> for LifeScenario {
    type Error = anyhow::Error;
    fn try_from(value: LifeConfig) -> Result<Self> {
        Self::new(value)
    }
}

impl From<LifeScenario> for LifeConfig {
    fn from(value: LifeScenario) -> Self {
        value.0
    }
}

impl LifeScenario {
    pub fn new(config: LifeConfig) -> Result<Self> {
        LifeRules::new(config.rules)?;
        let bounds = [
            (4..=256).contains(&config.width),
            (4..=256).contains(&config.height),
            config.density_percent <= 100,
            config
                .cooperator_percent
                .checked_add(config.exploiter_percent)
                .is_some_and(|total| total <= 100),
            (1..=128).contains(&config.food_capacity),
            config.initial_food <= config.food_capacity,
            (1..=1_000_000).contains(&config.initial_energy),
            (1..=128).contains(&config.harvest),
            (1..=32).contains(&config.metabolism),
            config.maturity_age < config.maximum_age,
            (1..=4096).contains(&config.maximum_age),
            (1..=1_000_000).contains(&config.birth_threshold),
            (1..=config.birth_threshold).contains(&config.child_energy),
        ];
        match bounds.into_iter().all(std::convert::identity) {
            true => Ok(Self(config)),
            false => Err(anyhow!(
                "life configuration exceeds grid, population, resource or lifecycle bounds"
            )),
        }
    }
    pub fn config(&self) -> &LifeConfig {
        &self.0
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    Cooperator,
    Exploiter,
    Conditional,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellLife {
    Vacant,
    Alive,
    Dead,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellAction {
    Rest,
    Cooperate,
    Exploit,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellEvent {
    None,
    Born,
    Died,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeCell {
    pub identity: u32,
    pub parent_identity: u32,
    pub strategy: Strategy,
    pub age: u32,
    pub energy: u32,
    pub food: u32,
    pub life: CellLife,
    pub action: CellAction,
    pub event: CellEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeSummary {
    pub population: u32,
    pub cooperators: u32,
    pub exploiters: u32,
    pub conditional: u32,
    pub births: u32,
    pub deaths: u32,
    pub hungry: u32,
    pub food: u64,
    pub energy: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeFrame {
    pub tick: u32,
    pub width: u32,
    pub height: u32,
    pub cells: Vec<LifeCell>,
    pub summary: LifeSummary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Brush {
    Cooperator,
    Exploiter,
    Conditional,
    Food,
    Erase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrushStroke {
    pub x: u32,
    pub y: u32,
    pub radius: u32,
    pub brush: Brush,
}
