use super::layout::{Agent as A, Firm as F, Settings as C};
use super::{cooperation, demography, employment, trade};
use cubecl::prelude::*;

#[cube]
pub fn read(agents: &[u32], slot: u32, field: usize) -> u32 {
    agents[slot as usize * A::WORDS + field]
}

#[cube]
pub fn get(agents: &mut [u32], slot: u32, field: usize) -> u32 {
    agents[slot as usize * A::WORDS + field]
}

#[cube]
pub fn put(agents: &mut [u32], slot: u32, field: usize, value: u32) {
    agents[slot as usize * A::WORDS + field] = value;
}

#[cube]
pub fn increase(agents: &mut [u32], slot: u32, field: usize, value: u32) {
    let updated = get(agents, slot, field) + value;
    put(agents, slot, field, updated);
}

#[cube]
pub fn need(agents: &mut [u32], slot: u32, config: &[u32]) -> u32 {
    select(
        get(agents, slot, A::LIFE) == 1,
        config[C::CHILD_NEED],
        config[C::NEED],
    )
}

#[cube]
pub fn living(life: u32) -> bool {
    life == 1 || life == 2
}

#[cube]
fn neighborhood(
    previous: &[u32],
    agents: &mut [u32],
    firms: &mut [u32],
    config: &[u32],
    group: u32,
    day: u32,
) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    let base = group as usize * F::WORDS;
    for slot in start..end {
        for field in 0..A::WORDS {
            let value = previous[slot as usize * A::WORDS + field];
            agents[slot as usize * A::WORDS + field] = value;
        }
        let life = get(agents, slot, A::LIFE);
        put(agents, slot, A::LIFE, select(life == 3, 0, life));
        for field in A::CONTRIBUTED..A::WORDS {
            put(agents, slot, field, 0);
        }
        let cooldown = get(agents, slot, A::COOLDOWN);
        put(agents, slot, A::COOLDOWN, cooldown - min(cooldown, 1));
    }
    for field in F::WORKERS..F::WORDS {
        firms[base + field] = 0;
    }
    employment::run(agents, firms, config, group, day);
    cooperation::run(previous, agents, config, group, day);
    trade::run(agents, firms, config, group, day);
    demography::run(agents, firms, config, group, day);
}

#[cube(launch)]
pub fn step(previous: &[u32], agents: &mut [u32], firms: &mut [u32], config: &[u32], day: u32) {
    let group = ABSOLUTE_POS as u32;
    let groups = config[C::CAPACITY].div_ceil(config[C::SIZE]);
    if group < groups {
        neighborhood(previous, agents, firms, config, group, day);
    }
}
