use super::Device;
use crate::{arithmetic::DrawKey, domain::life::*};
use anyhow::{Context, Result, anyhow};
use burn_wgpu::CubeTensor;
use bytemuck::{Pod, Zeroable};
use cubecl::{CubeCount, CubeDim, prelude::BufferArg};
use std::mem::offset_of;

mod kernels;

macro_rules! layout {
    ($name:ident { $($field:ident: $constant:ident),* $(,)? }) => {
        #[repr(C)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        struct $name { $(pub $field: u32),* }
        impl $name {
            const WORDS: usize = size_of::<Self>() / 4;
            $(const $constant: usize = offset_of!(Self, $field) / 4;)*
        }
    };
}

layout!(Cell {
    identity: ID,
    parent: PARENT,
    strategy: STRATEGY,
    age: AGE,
    energy: ENERGY,
    food: FOOD,
    life: LIFE,
    action: ACTION,
    event: EVENT,
    contribution: CONTRIBUTION,
    claim: CLAIM,
    regrown: REGROWN,
    harvested: HARVESTED,
    generated: GENERATED,
    spent: SPENT,
    discarded: DISCARDED,
    hungry: HUNGRY,
});

layout!(Parameters {
    width: WIDTH,
    height: HEIGHT,
    food_capacity: FOOD_CAPACITY,
    harvest: HARVEST,
    metabolism: METABOLISM,
    maturity: MATURITY,
    maximum_age: MAXIMUM_AGE,
    birth_threshold: BIRTH_THRESHOLD,
    child_energy: CHILD_ENERGY,
    regrowth: REGROWTH,
    cooperation: COOPERATION,
    exploitation: EXPLOITATION,
    mutation: MUTATION,
    seed: SEED,
});

impl Parameters {
    fn new(config: &LifeConfig) -> Self {
        Self {
            width: config.width,
            height: config.height,
            food_capacity: config.food_capacity,
            harvest: config.harvest,
            metabolism: config.metabolism,
            maturity: config.maturity_age,
            maximum_age: config.maximum_age,
            birth_threshold: config.birth_threshold,
            child_energy: config.child_energy,
            regrowth: config.rules.regrowth,
            cooperation: config.rules.cooperation,
            exploitation: config.rules.exploitation,
            mutation: config.rules.mutation_per_mille,
            seed: config.seed as u32 ^ (config.seed >> 32) as u32,
        }
    }
}

pub struct LifeSimulation {
    scenario: LifeScenario,
    device: Device,
    state: CubeTensor,
    prepared: CubeTensor,
    settled: CubeTensor,
    candidate: CubeTensor,
    parameters: CubeTensor,
    frame: LifeFrame,
    epoch: u32,
}

impl LifeSimulation {
    pub fn new(scenario: LifeScenario, device: Device) -> Result<Self> {
        let c = scenario.config();
        let count = c.width * c.height;
        let bytes = u64::from(count) * Cell::WORDS as u64 * 4 * 4 + (Parameters::WORDS * 4) as u64;
        match bytes <= device.memory_budget.bytes() {
            true => Ok(()),
            false => Err(anyhow!("life buffers exceed GPU memory budget")),
        }?;
        let cells = (0..count)
            .map(|index| {
                let draw = |purpose| {
                    DrawKey {
                        seed: c.seed,
                        day: 0,
                        phase: 0,
                        actor: u64::from(index),
                        purpose,
                        index: 0,
                    }
                    .draw()
                };
                let present = draw(0x4c494600) % 100 < u64::from(c.density_percent);
                let kind = draw(0x4c494601) % 100;
                let strategy = match kind {
                    value if value < u64::from(c.cooperator_percent) => Strategy::Cooperator,
                    value if value < u64::from(c.cooperator_percent + c.exploiter_percent) => {
                        Strategy::Exploiter
                    }
                    _ => Strategy::Conditional,
                };
                Cell {
                    identity: match present {
                        true => index + 1,
                        false => 0,
                    },
                    strategy: strategy as u32,
                    age: match present {
                        true => (draw(0x4c494602) % u64::from((c.maximum_age / 2).max(1))) as u32,
                        false => 0,
                    },
                    energy: match present {
                        true => c.initial_energy,
                        false => 0,
                    },
                    food: c.initial_food,
                    life: match present {
                        true => CellLife::Alive,
                        false => CellLife::Vacant,
                    } as u32,
                    action: match (present, strategy) {
                        (true, Strategy::Exploiter) => CellAction::Exploit,
                        (true, _) => CellAction::Cooperate,
                        _ => CellAction::Rest,
                    } as u32,
                    ..Cell::zeroed()
                }
            })
            .collect::<Vec<_>>();
        let state = device.tensor(bytemuck::cast_slice(&cells));
        let prepared = device.empty(count as usize * Cell::WORDS);
        let settled = device.empty(count as usize * Cell::WORDS);
        let candidate = device.empty(count as usize * Cell::WORDS);
        let parameters = device.tensor(bytemuck::cast_slice(&[Parameters::new(c)]));
        let frame = Self::frame(c, 0, &cells)?;
        Ok(Self {
            scenario,
            device,
            state,
            prepared,
            settled,
            candidate,
            parameters,
            frame,
            epoch: 0,
        })
    }

    pub fn snapshot(&self) -> LifeFrame {
        self.frame.clone()
    }

    fn next_epoch(&self) -> Result<u32> {
        let count = self.frame.width * self.frame.height;
        self.epoch
            .checked_add(1)
            .filter(|epoch| {
                epoch
                    .checked_mul(count)
                    .and_then(|base| base.checked_add(count))
                    .is_some()
            })
            .context("agent identity space exhausted; reset the simulation")
    }

    pub fn advance(&mut self) -> Result<LifeFrame> {
        let epoch = self.next_epoch()?;
        let tick = self
            .frame
            .tick
            .checked_add(1)
            .context("life tick overflow")?;
        let count = self.frame.width * self.frame.height;
        let argument = |tensor: &CubeTensor| unsafe {
            BufferArg::from_raw_parts(tensor.handle.clone(), tensor.meta.shape()[0])
        };
        let dispatch = CubeCount::Static(count.div_ceil(self.device.workgroup), 1, 1);
        let dimensions = CubeDim::new_1d(self.device.workgroup);
        kernels::prepare::launch(
            &self.device.client,
            dispatch.clone(),
            dimensions,
            argument(&self.state),
            argument(&self.prepared),
            argument(&self.parameters),
        );
        kernels::interact::launch(
            &self.device.client,
            dispatch.clone(),
            dimensions,
            argument(&self.prepared),
            argument(&self.settled),
            argument(&self.parameters),
            tick,
        );
        kernels::reproduce::launch(
            &self.device.client,
            dispatch,
            dimensions,
            argument(&self.settled),
            argument(&self.candidate),
            argument(&self.parameters),
            tick,
            epoch,
        );
        let words = self.device.read(&self.candidate)?;
        let cells = bytemuck::cast_slice::<u32, Cell>(&words);
        let frame = Self::frame(self.scenario.config(), tick, cells)?;
        let sum =
            |field: fn(&Cell) -> u32| cells.iter().map(|cell| u64::from(field(cell))).sum::<u64>();
        let accounts = [
            self.frame.summary.food + sum(|cell| cell.regrown)
                == frame.summary.food + sum(|cell| cell.harvested),
            self.frame.summary.energy + sum(|cell| cell.harvested) + sum(|cell| cell.generated)
                == frame.summary.energy + sum(|cell| cell.spent) + sum(|cell| cell.discarded),
            self.frame.summary.population + frame.summary.births
                == frame.summary.population + frame.summary.deaths,
        ];
        match accounts.into_iter().all(std::convert::identity) {
            true => Ok(()),
            false => Err(anyhow!(
                "life resource or population audit failed at tick {tick}"
            )),
        }?;
        std::mem::swap(&mut self.state, &mut self.candidate);
        self.frame = frame;
        self.epoch = epoch;
        Ok(self.snapshot())
    }

    pub fn set_rules(&mut self, rules: LifeRules) -> Result<()> {
        let mut config = self.scenario.config().clone();
        config.rules = LifeRules::new(rules)?;
        let scenario = LifeScenario::new(config)?;
        self.device.write(
            &self.parameters,
            bytemuck::cast_slice(&[Parameters::new(scenario.config())]),
        );
        self.scenario = scenario;
        Ok(())
    }

    pub fn reset(&mut self, scenario: LifeScenario) -> Result<()> {
        let device = Device {
            client: self.device.client.clone(),
            cube: self.device.cube.clone(),
            name: self.device.name.clone(),
            workgroup: self.device.workgroup,
            memory_budget: self.device.memory_budget,
        };
        *self = Self::new(scenario, device)?;
        Ok(())
    }

    pub fn paint(&mut self, stroke: BrushStroke) -> Result<()> {
        let c = self.scenario.config();
        match [stroke.x < c.width, stroke.y < c.height, stroke.radius <= 16]
            .into_iter()
            .all(std::convert::identity)
        {
            true => Ok(()),
            false => Err(anyhow!(
                "brush coordinates must be on the map and radius must be 0..=16"
            )),
        }?;
        let epoch = self.next_epoch()?;
        let count = c.width * c.height;
        let cells = self
            .frame
            .cells
            .iter()
            .enumerate()
            .map(|(index, old)| {
                let mut cell = Cell {
                    identity: old.identity,
                    parent: old.parent_identity,
                    strategy: old.strategy as u32,
                    age: old.age,
                    energy: old.energy,
                    food: old.food,
                    life: old.life as u32,
                    action: old.action as u32,
                    ..Cell::zeroed()
                };
                let dx = (index as u32 % c.width).abs_diff(stroke.x);
                let dy = (index as u32 / c.width).abs_diff(stroke.y);
                let distance = dx.min(c.width - dx).pow(2) + dy.min(c.height - dy).pow(2);
                match (distance <= stroke.radius.pow(2), stroke.brush) {
                    (true, Brush::Food) => cell.food = c.food_capacity,
                    (true, Brush::Erase) => {
                        cell = Cell {
                            food: cell.food,
                            event: match old.life {
                                CellLife::Alive => CellEvent::Died,
                                _ => CellEvent::None,
                            } as u32,
                            ..Cell::zeroed()
                        };
                    }
                    (true, brush) => {
                        let strategy = match brush {
                            Brush::Exploiter => Strategy::Exploiter,
                            Brush::Conditional => Strategy::Conditional,
                            _ => Strategy::Cooperator,
                        };
                        cell = Cell {
                            identity: epoch * count + index as u32 + 1,
                            strategy: strategy as u32,
                            age: c.maturity_age,
                            energy: c.initial_energy,
                            food: cell.food,
                            life: CellLife::Alive as u32,
                            event: CellEvent::Born as u32,
                            action: match strategy {
                                Strategy::Exploiter => CellAction::Exploit,
                                _ => CellAction::Cooperate,
                            } as u32,
                            ..Cell::zeroed()
                        };
                    }
                    _ => {}
                }
                cell
            })
            .collect::<Vec<_>>();
        let frame = Self::frame(c, self.frame.tick, &cells)?;
        self.device.write(&self.state, bytemuck::cast_slice(&cells));
        self.frame = frame;
        self.epoch = epoch;
        Ok(())
    }

    fn frame(config: &LifeConfig, tick: u32, raw: &[Cell]) -> Result<LifeFrame> {
        let cells = raw
            .iter()
            .map(|cell| {
                Ok(LifeCell {
                    identity: cell.identity,
                    parent_identity: cell.parent,
                    strategy: match cell.strategy {
                        0 => Ok(Strategy::Cooperator),
                        1 => Ok(Strategy::Exploiter),
                        2 => Ok(Strategy::Conditional),
                        _ => Err(anyhow!("invalid life strategy")),
                    }?,
                    age: cell.age,
                    energy: cell.energy,
                    food: cell.food,
                    life: match cell.life {
                        0 => Ok(CellLife::Vacant),
                        1 => Ok(CellLife::Alive),
                        2 => Ok(CellLife::Dead),
                        _ => Err(anyhow!("invalid cell life")),
                    }?,
                    action: match cell.action {
                        0 => Ok(CellAction::Rest),
                        1 => Ok(CellAction::Cooperate),
                        2 => Ok(CellAction::Exploit),
                        _ => Err(anyhow!("invalid cell action")),
                    }?,
                    event: match cell.event {
                        0 => Ok(CellEvent::None),
                        1 => Ok(CellEvent::Born),
                        2 => Ok(CellEvent::Died),
                        _ => Err(anyhow!("invalid cell event")),
                    }?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let live = |strategy| {
            cells
                .iter()
                .filter(|cell| cell.life == CellLife::Alive && cell.strategy == strategy)
                .count() as u32
        };
        let summary = LifeSummary {
            population: cells
                .iter()
                .filter(|cell| cell.life == CellLife::Alive)
                .count() as u32,
            cooperators: live(Strategy::Cooperator),
            exploiters: live(Strategy::Exploiter),
            conditional: live(Strategy::Conditional),
            births: cells
                .iter()
                .filter(|cell| cell.event == CellEvent::Born)
                .count() as u32,
            deaths: cells
                .iter()
                .filter(|cell| cell.event == CellEvent::Died)
                .count() as u32,
            hungry: raw.iter().filter(|cell| cell.hungry > 0).count() as u32,
            food: cells.iter().map(|cell| u64::from(cell.food)).sum(),
            energy: cells.iter().map(|cell| u64::from(cell.energy)).sum(),
        };
        Ok(LifeFrame {
            tick,
            width: config.width,
            height: config.height,
            cells,
            summary,
        })
    }
}
