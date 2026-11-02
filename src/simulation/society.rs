use super::Device;
use crate::domain::society::*;
use anyhow::{Result, anyhow};
use burn_wgpu::CubeTensor;
use bytemuck::Zeroable;
use cubecl::{CubeCount, CubeDim, prelude::BufferArg};
use serde::Serialize;

mod cooperation;
mod demography;
mod employment;
mod kernels;
mod layout;
mod trade;
use layout::{Agent, Firm, Settings};

pub struct SocietySimulation {
    scenario: SocietyScenario,
    device: Device,
    state: CubeTensor,
    candidate: CubeTensor,
    firms: CubeTensor,
    config: CubeTensor,
    firm_state: Vec<Firm>,
    day: u32,
    food: u64,
    cash: u64,
    population: u32,
}

impl SocietySimulation {
    pub fn new(scenario: SocietyScenario, device: Device) -> Result<Self> {
        let c = scenario.config();
        let mut agents = vec![Agent::zeroed(); c.capacity as usize];
        for seed in &c.agents {
            agents[seed.slot as usize] = Agent {
                identity: seed.slot + 1,
                strategy: seed.strategy as u32,
                trade: seed.trade_behavior as u32,
                life: match seed.age >= c.adult_age {
                    true => Life::Adult,
                    false => Life::Child,
                } as u32,
                age: seed.age,
                food: c.initial_food,
                cash: c.initial_cash,
                cooldown: c.birth_interval,
                action: match seed.strategy {
                    Strategy::FreeRider => Action::FreeRide,
                    _ => Action::Contribute,
                } as u32,
                ..Agent::zeroed()
            };
        }
        let firm_state = vec![
            Firm {
                cash: c.firm_cash,
                food: c.firm_food,
                wage: c.wage_offer,
                ..Firm::zeroed()
            };
            c.capacity.div_ceil(c.neighborhood_size) as usize
        ];
        let settings = Settings {
            capacity: c.capacity,
            size: c.neighborhood_size,
            effort: c.daily_effort,
            project: c.project_effort,
            productivity: c.project_productivity,
            need: c.daily_food_need,
            child_need: c.child_food_need,
            threshold: c.reciprocity_percent,
            sharing: c.sharing as u32,
            trade_rule: c.trade_rule as u32,
            price: c.food_price,
            jobs: c.jobs,
            work: c.work_effort,
            work_food: c.work_food,
            maximum_wage: c.maximum_wage,
            reservation: c.reservation_wage,
            wage_rule: c.wage_rule as u32,
            adult_age: c.adult_age,
            starvation: c.starvation_days,
            birth_rule: c.birth_rule as u32,
            birth_food: c.birth_food,
            birth_interval: c.birth_interval,
        };
        let bytes =
            (agents.len() * Agent::WORDS * 2 + firm_state.len() * Firm::WORDS + Settings::WORDS)
                * 4;
        match bytes as u64 <= device.memory_budget.bytes() {
            true => Ok(()),
            false => Err(anyhow!("society buffers exceed GPU memory budget")),
        }?;
        let state = device.tensor(bytemuck::cast_slice(&agents));
        let candidate = device.empty(agents.len() * Agent::WORDS);
        let firms = device.tensor(bytemuck::cast_slice(&firm_state));
        let config = device.tensor(bytemuck::cast_slice(&[settings]));
        let food = u64::from(c.initial_food) * c.agents.len() as u64
            + u64::from(c.firm_food) * firm_state.len() as u64;
        let cash = u64::from(c.initial_cash) * c.agents.len() as u64
            + u64::from(c.firm_cash) * firm_state.len() as u64;
        let population = c.agents.len() as u32;
        Ok(Self {
            scenario,
            device,
            state,
            candidate,
            firms,
            config,
            firm_state,
            day: 0,
            food,
            cash,
            population,
        })
    }

    pub fn advance(&mut self) -> Result<SocietyRound> {
        match self.day < MAX_DAYS {
            true => Ok(()),
            false => Err(anyhow!("society is limited to {MAX_DAYS} days")),
        }?;
        self.device
            .write(&self.firms, bytemuck::cast_slice(&self.firm_state));
        let argument = |tensor: &CubeTensor| unsafe {
            BufferArg::from_raw_parts(tensor.handle.clone(), tensor.meta.shape()[0])
        };
        kernels::step::launch(
            &self.device.client,
            CubeCount::Static(
                (self.firm_state.len() as u32).div_ceil(self.device.workgroup),
                1,
                1,
            ),
            CubeDim::new_1d(self.device.workgroup),
            argument(&self.state),
            argument(&self.candidate),
            argument(&self.firms),
            argument(&self.config),
            self.day,
        );
        let agent_words = self.device.read(&self.candidate)?;
        let agents = bytemuck::cast_slice::<u32, Agent>(&agent_words);
        let firm_words = self.device.read(&self.firms)?;
        let firms = bytemuck::cast_slice::<u32, Firm>(&firm_words);
        let round = self.observation(agents, firms)?;
        let accounts = [
            self.food + round.summary.produced_food
                == round.summary.food_reserves
                    + round.summary.consumed_food
                    + round.summary.abandoned_food,
            self.cash == round.summary.cash,
            round.summary.population + round.summary.deaths
                == self.population + round.summary.births,
        ];
        match accounts.into_iter().all(std::convert::identity) {
            true => Ok(()),
            false => Err(anyhow!(
                "society food, cash or population audit failed on day {}",
                self.day + 1
            )),
        }?;
        std::mem::swap(&mut self.state, &mut self.candidate);
        self.firm_state = firms.to_vec();
        self.day += 1;
        self.food = round.summary.food_reserves;
        self.population = round.summary.population;
        Ok(round)
    }

    fn observation(&self, agents: &[Agent], firms: &[Firm]) -> Result<SocietyRound> {
        let c = self.scenario.config();
        let sum = |field: fn(&Agent) -> u32| {
            agents
                .iter()
                .map(|agent| u64::from(field(agent)))
                .sum::<u64>()
        };
        let firm_sum =
            |field: fn(&Firm) -> u32| firms.iter().map(|firm| u64::from(field(firm))).sum::<u64>();
        let count = |life| agents.iter().filter(|agent| agent.life == life).count() as u32;
        let summary = RoundSummary {
            day: self.day + 1,
            population: count(1) + count(2),
            adults: count(2),
            children: count(1),
            contributors: agents.iter().filter(|agent| agent.action == 2).count() as u32,
            employed: sum(|agent| agent.employed) as u32,
            hungry_agents: agents.iter().filter(|agent| agent.shortfall > 0).count() as u32,
            births: firm_sum(|firm| firm.births) as u32,
            deaths: firm_sum(|firm| firm.deaths) as u32,
            trades: firm_sum(|firm| firm.trades) as u32,
            frauds: firm_sum(|firm| firm.frauds) as u32,
            food_reserves: sum(|agent| agent.food) + firm_sum(|firm| firm.food),
            cash: sum(|agent| agent.cash) + firm_sum(|firm| firm.cash),
            produced_food: sum(|agent| agent.private)
                + sum(|agent| agent.shared)
                + firm_sum(|firm| firm.produced),
            consumed_food: sum(|agent| agent.consumed),
            abandoned_food: firm_sum(|firm| firm.abandoned),
        };
        let outcomes = agents
            .iter()
            .enumerate()
            .map(|(slot, agent)| {
                let strategy = match agent.strategy {
                    0 => Ok(Strategy::Cooperator),
                    1 => Ok(Strategy::FreeRider),
                    2 => Ok(Strategy::Reciprocator),
                    3 => Ok(Strategy::Imitator),
                    _ => Err(anyhow!("unknown society strategy")),
                }?;
                let trade_behavior = match agent.trade {
                    0 => Ok(TradeBehavior::Fair),
                    1 => Ok(TradeBehavior::Cheat),
                    _ => Err(anyhow!("unknown trade behavior")),
                }?;
                let life = match agent.life {
                    0 => Ok(Life::Vacant),
                    1 => Ok(Life::Child),
                    2 => Ok(Life::Adult),
                    3 => Ok(Life::Dead),
                    _ => Err(anyhow!("unknown life state")),
                }?;
                let action = match agent.action {
                    0 => Ok(Action::Rest),
                    1 => Ok(Action::FreeRide),
                    2 => Ok(Action::Contribute),
                    _ => Err(anyhow!("unknown social action")),
                }?;
                Ok(AgentOutcome {
                    slot: slot as u32,
                    identity: agent.identity,
                    parent_identity: agent.parent,
                    neighborhood: slot as u32 / c.neighborhood_size + 1,
                    strategy,
                    trade_behavior,
                    life,
                    age: agent.age,
                    action,
                    food: agent.food,
                    cash: agent.cash,
                    contributed_effort: agent.contributed,
                    private_food: agent.private,
                    shared_food: agent.shared,
                    wage: agent.wage,
                    reservation_wage: agent.reservation,
                    employed: agent.employed == 1,
                    food_shortfall: agent.shortfall,
                    consecutive_shortages: agent.hunger,
                    bought_food: agent.bought,
                    sold_food: agent.sold,
                    cheated_trades: agent.cheated,
                    refused_trades: agent.refused,
                    frauds_committed: agent.frauds,
                    last_fraud_identity: agent.last_fraud,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SocietyRound {
            summary,
            agents: outcomes,
            firms: firms
                .iter()
                .enumerate()
                .map(|(group, firm)| FirmOutcome {
                    neighborhood: group as u32 + 1,
                    cash: firm.cash,
                    food: firm.food,
                    wage_offer: firm.wage,
                    workers: firm.workers,
                    vacancies: c.jobs - firm.workers,
                    produced_food: firm.produced,
                })
                .collect(),
        })
    }
}

#[derive(Serialize)]
pub struct SocietyReport {
    pub model: &'static str,
    pub device: String,
    pub workgroup: u32,
    pub scenario: SocietyScenario,
    pub rounds: Vec<SocietyRound>,
}

impl SocietyReport {
    pub fn run(scenario: SocietyScenario, device: Device, days: u32) -> Result<Self> {
        let bounds = [
            (1..=MAX_DAYS).contains(&days),
            u64::from(scenario.config().capacity) * u64::from(days) <= 262_144,
        ];
        match bounds.into_iter().all(std::convert::identity) {
            true => Ok(()),
            false => Err(anyhow!(
                "society reports need 1..={MAX_DAYS} days and at most 262144 slot-days"
            )),
        }?;
        let name = device.name.clone();
        let workgroup = device.workgroup;
        let mut simulation = SocietySimulation::new(scenario.clone(), device)?;
        let rounds = (0..days)
            .map(|_| simulation.advance())
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            model: "bad-econ/society/v1",
            device: name,
            workgroup,
            scenario,
            rounds,
        })
    }
}
