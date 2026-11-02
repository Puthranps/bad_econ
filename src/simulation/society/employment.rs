use super::{
    kernels::*,
    layout::{Agent as A, Firm as F, Settings as C},
};
use cubecl::prelude::*;

#[cube]
fn hire(agents: &mut [u32], firms: &mut [u32], config: &[u32], group: u32, slot: u32) {
    let base = group as usize * F::WORDS;
    let reservation = max(
        1,
        config[C::RESERVATION] - min(config[C::RESERVATION], get(agents, slot, A::HUNGER)),
    );
    put(agents, slot, A::RESERVATION, reservation);
    let posted = firms[base + F::WAGE];
    let wage = select(
        config[C::WAGE_RULE] == 0,
        min(config[C::MAXIMUM_WAGE], max(posted, reservation)),
        posted,
    );
    let accepted = wage >= reservation
        && firms[base + F::CASH] >= wage
        && firms[base + F::WORKERS] < config[C::JOBS];
    if accepted {
        firms[base + F::CASH] -= wage;
        firms[base + F::WORKERS] += 1;
        firms[base + F::FOOD] += config[C::WORK_FOOD];
        firms[base + F::PRODUCED] += config[C::WORK_FOOD];
        increase(agents, slot, A::CASH, wage);
        put(agents, slot, A::WAGE, wage);
        put(agents, slot, A::EMPLOYED, 1);
    }
}

#[cube]
pub fn run(agents: &mut [u32], firms: &mut [u32], config: &[u32], group: u32, day: u32) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    for offset in 0..end - start {
        let slot = start + (offset + day) % (end - start);
        if get(agents, slot, A::LIFE) == 2 {
            hire(agents, firms, config, group, slot);
        }
    }
}
