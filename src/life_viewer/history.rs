use crate::domain::life::*;
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Totals {
    pub advanced_ticks: u64,
    pub natural_births: u64,
    pub natural_deaths: u64,
    pub hungry_agent_ticks: u64,
    pub peak_population: u32,
    pub manual_strokes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Sample {
    pub tick: u32,
    pub population: u32,
    pub cooperators: u32,
    pub exploiters: u32,
    pub conditional: u32,
    pub natural_births: u32,
    pub natural_deaths: u32,
    pub hungry: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) enum Cause {
    Natural,
    Manual,
}

#[derive(Clone, Debug, Serialize)]
pub(super) enum EventKind {
    Started {
        seed: u64,
    },
    Rules {
        rules: LifeRules,
    },
    Painted {
        stroke: BrushStroke,
        population_change: i64,
    },
    Extinction {
        cause: Cause,
    },
    StrategyLost {
        strategy: Strategy,
        cause: Cause,
    },
    StrategyAppeared {
        strategy: Strategy,
        cause: Cause,
    },
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Event {
    pub tick: u32,
    pub kind: EventKind,
}
impl Event {
    pub fn label(&self) -> String {
        let detail = match &self.kind {
            EventKind::Started { seed } => format!("Started seed {seed}"),
            EventKind::Rules { rules } => format!(
                "Rules: growth {} / coop {} / exploit {} / mutation {:.1}%",
                rules.regrowth,
                rules.cooperation,
                rules.exploitation,
                rules.mutation_per_mille as f32 / 10.0
            ),
            EventKind::Painted {
                stroke,
                population_change,
            } => format!(
                "Paint {:?} at {},{} r{}; population {population_change:+}",
                stroke.brush, stroke.x, stroke.y, stroke.radius
            ),
            EventKind::Extinction { cause } => format!("Population reached zero ({cause:?})"),
            EventKind::StrategyLost { strategy, cause } => {
                format!("{strategy:?} population reached zero ({cause:?})")
            }
            EventKind::StrategyAppeared { strategy, cause } => {
                format!("{strategy:?} appeared ({cause:?})")
            }
        };
        format!("t {:>6}   {detail}", self.tick)
    }
}

#[derive(Clone, Serialize)]
pub(super) struct RunHistory {
    pub initial_config: LifeConfig,
    pub totals: Totals,
    pub samples: Vec<Sample>,
    pub sample_stride: u32,
    pub events: VecDeque<Event>,
    pub discarded_events: u64,
}

impl RunHistory {
    pub fn new(config: LifeConfig) -> Self {
        Self {
            initial_config: config,
            totals: Totals::default(),
            samples: Vec::new(),
            sample_stride: 1,
            events: VecDeque::new(),
            discarded_events: 0,
        }
    }

    pub fn start(&mut self, frame: &LifeFrame) {
        self.event(
            frame.tick,
            EventKind::Started {
                seed: self.initial_config.seed,
            },
        );
        self.sample(frame, Cause::Manual);
    }

    pub fn advance(&mut self, frame: &LifeFrame) {
        self.totals.advanced_ticks += 1;
        self.totals.natural_births += u64::from(frame.summary.births);
        self.totals.natural_deaths += u64::from(frame.summary.deaths);
        self.totals.hungry_agent_ticks += u64::from(frame.summary.hungry);
        self.transitions(frame, Cause::Natural);
        self.sample(frame, Cause::Natural);
    }

    pub fn paint(&mut self, frame: &LifeFrame, stroke: BrushStroke) {
        let previous = self
            .samples
            .last()
            .map(|point| point.population)
            .unwrap_or(0);
        self.totals.manual_strokes += 1;
        self.event(
            frame.tick,
            EventKind::Painted {
                stroke,
                population_change: i64::from(frame.summary.population) - i64::from(previous),
            },
        );
        self.transitions(frame, Cause::Manual);
        self.sample(frame, Cause::Manual);
    }

    pub fn rules(&mut self, tick: u32, rules: LifeRules) {
        self.event(tick, EventKind::Rules { rules });
    }

    fn event(&mut self, tick: u32, kind: EventKind) {
        self.events.push_back(Event { tick, kind });
        if self.events.len() > 64 {
            self.events.pop_front();
            self.discarded_events += 1;
        }
    }

    fn transitions(&mut self, frame: &LifeFrame, cause: Cause) {
        let previous = self.samples.last().cloned();
        if let Some(previous) = previous {
            if let (1.., 0) = (previous.population, frame.summary.population) {
                self.event(frame.tick, EventKind::Extinction { cause });
            }
            for strategy in [
                Strategy::Cooperator,
                Strategy::Exploiter,
                Strategy::Conditional,
            ] {
                let before = match strategy {
                    Strategy::Cooperator => previous.cooperators,
                    Strategy::Exploiter => previous.exploiters,
                    Strategy::Conditional => previous.conditional,
                };
                let after = match strategy {
                    Strategy::Cooperator => frame.summary.cooperators,
                    Strategy::Exploiter => frame.summary.exploiters,
                    Strategy::Conditional => frame.summary.conditional,
                };
                match (before, after) {
                    (1.., 0) => self.event(frame.tick, EventKind::StrategyLost { strategy, cause }),
                    (0, 1..) => {
                        self.event(frame.tick, EventKind::StrategyAppeared { strategy, cause })
                    }
                    _ => (),
                }
            }
        }
    }

    fn sample(&mut self, frame: &LifeFrame, cause: Cause) {
        self.totals.peak_population = self.totals.peak_population.max(frame.summary.population);
        let last = self.samples.last();
        let natural_births = match cause {
            Cause::Natural => frame.summary.births,
            Cause::Manual => last
                .filter(|point| point.tick == frame.tick)
                .map(|point| point.natural_births)
                .unwrap_or(0),
        };
        let natural_deaths = match cause {
            Cause::Natural => frame.summary.deaths,
            Cause::Manual => last
                .filter(|point| point.tick == frame.tick)
                .map(|point| point.natural_deaths)
                .unwrap_or(0),
        };
        let hungry = match cause {
            Cause::Natural => frame.summary.hungry,
            Cause::Manual => last
                .filter(|point| point.tick == frame.tick)
                .map(|point| point.hungry)
                .unwrap_or(0),
        };
        let replace = last
            .is_some_and(|point| point.tick == frame.tick || point.tick % self.sample_stride != 0);
        if let (2.., true) = (self.samples.len(), replace) {
            self.samples.pop();
        }
        self.samples.push(Sample {
            tick: frame.tick,
            population: frame.summary.population,
            cooperators: frame.summary.cooperators,
            exploiters: frame.summary.exploiters,
            conditional: frame.summary.conditional,
            natural_births,
            natural_deaths,
            hungry,
        });
        if self.samples.len() > 512 {
            self.sample_stride = self.sample_stride.saturating_mul(2);
            let latest = self.samples.len() - 1;
            self.samples = self
                .samples
                .drain(..)
                .enumerate()
                .filter(|(index, sample)| {
                    *index == 0 || *index == latest || sample.tick % self.sample_stride == 0
                })
                .map(|(_, sample)| sample)
                .collect();
        }
    }

    pub fn export(&self, frame: &LifeFrame, rules: LifeRules, sequence: u64) -> Result<PathBuf> {
        std::fs::create_dir_all("target/life")?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock precedes epoch")?
            .as_nanos();
        let path = PathBuf::from(format!(
            "target/life/history-{}-{}-{}-{stamp}-{sequence}.json",
            self.initial_config.seed,
            frame.tick,
            std::process::id()
        ));
        let record = serde_json::json!({"format":"bad-econ/life-history/v1", "history":self, "current_rules":rules, "last_frame":frame,
            "sampling":"Whole-run tick snapshots, coarsened by power-of-two stride; first and latest retained. Manual edits at tick zero may create a vertical jump from the initial state. Intermediate peaks may be absent from sampled curves. Flow values are from the latest natural tick; paint is excluded from flow totals. Event log retains the latest 64."});
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&serde_json::to_vec_pretty(&record)?)?;
        Ok(path)
    }
}
