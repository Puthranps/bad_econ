use super::{
    kernels::*,
    layout::{Agent as A, Firm as F, Settings as C},
};
use cubecl::prelude::*;

#[cube]
fn support(agents: &mut [u32], config: &[u32], child: u32, start: u32, end: u32) {
    for parent in start..end {
        let related = get(agents, parent, A::ID) == get(agents, child, A::PARENT)
            && get(agents, parent, A::LIFE) == 2;
        let shortage =
            config[C::CHILD_NEED] - min(config[C::CHILD_NEED], get(agents, child, A::FOOD));
        let available =
            get(agents, parent, A::FOOD) - min(get(agents, parent, A::FOOD), config[C::NEED]);
        let transfer = select(related, min(shortage, available), 0);
        let food = get(agents, parent, A::FOOD) - transfer;
        put(agents, parent, A::FOOD, food);
        increase(agents, child, A::FOOD, transfer);
    }
}

#[cube]
fn consume(agents: &mut [u32], firms: &mut [u32], config: &[u32], group: u32, slot: u32) {
    let required = need(agents, slot, config);
    let consumed = min(required, get(agents, slot, A::FOOD));
    let food = get(agents, slot, A::FOOD) - consumed;
    let hunger = select(consumed < required, get(agents, slot, A::HUNGER) + 1, 0);
    let age = get(agents, slot, A::AGE) + 1;
    let died = hunger >= config[C::STARVATION];
    let life = select(died, 3u32, select(age >= config[C::ADULT_AGE], 2u32, 1u32));
    let abandoned = select(died, food, 0);
    let estate = select(died, get(agents, slot, A::CASH), 0);
    let cash = get(agents, slot, A::CASH) - estate;
    put(agents, slot, A::FOOD, food - abandoned);
    put(agents, slot, A::CASH, cash);
    put(agents, slot, A::CONSUMED, consumed);
    put(agents, slot, A::SHORTFALL, required - consumed);
    put(agents, slot, A::HUNGER, hunger);
    put(agents, slot, A::AGE, age);
    put(agents, slot, A::LIFE, life);
    let base = group as usize * F::WORDS;
    firms[base + F::CASH] += estate;
    firms[base + F::ABANDONED] += abandoned;
    firms[base + F::DEATHS] += u32::cast_from(died);
}

#[cube]
fn birth(
    agents: &mut [u32],
    firms: &mut [u32],
    config: &[u32],
    group: u32,
    parent: u32,
    child: u32,
    day: u32,
) {
    let cash = get(agents, parent, A::CASH) / 4;
    let remaining_cash = get(agents, parent, A::CASH) - cash;
    let food = get(agents, parent, A::FOOD) - config[C::BIRTH_FOOD];
    let identity = get(agents, parent, A::ID);
    let strategy = get(agents, parent, A::STRATEGY);
    let trade = get(agents, parent, A::TRADE);
    put(agents, parent, A::CASH, remaining_cash);
    put(agents, parent, A::FOOD, food);
    put(agents, parent, A::COOLDOWN, config[C::BIRTH_INTERVAL]);
    for field in 0..A::WORDS {
        put(agents, child, field, 0);
    }
    put(
        agents,
        child,
        A::ID,
        (day + 1) * config[C::CAPACITY] + child + 1,
    );
    put(agents, child, A::PARENT, identity);
    put(agents, child, A::STRATEGY, strategy);
    put(agents, child, A::TRADE, trade);
    put(agents, child, A::LIFE, 1);
    put(agents, child, A::FOOD, config[C::BIRTH_FOOD]);
    put(agents, child, A::CASH, cash);
    put(agents, child, A::COOLDOWN, config[C::BIRTH_INTERVAL]);
    firms[group as usize * F::WORDS + F::BIRTHS] += 1;
}

#[cube]
fn reproduce(agents: &mut [u32], firms: &mut [u32], config: &[u32], group: u32, day: u32) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    let missing = config[C::CAPACITY];
    let mut parent = missing;
    let mut child = missing;
    for offset in 0..end - start {
        let slot = start + (offset + day) % (end - start);
        let eligible = get(agents, slot, A::LIFE) == 2
            && get(agents, slot, A::COOLDOWN) == 0
            && get(agents, slot, A::FOOD) >= config[C::BIRTH_FOOD] + 2 * config[C::NEED];
        parent = select(eligible && parent == missing, slot, parent);
        child = select(
            get(agents, slot, A::LIFE) == 0 && child == missing,
            slot,
            child,
        );
    }
    let ready = parent < missing && child < missing;
    if ready {
        birth(agents, firms, config, group, parent, child, day);
    }
}

#[cube]
pub fn run(agents: &mut [u32], firms: &mut [u32], config: &[u32], group: u32, day: u32) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    for slot in start..end {
        if get(agents, slot, A::LIFE) == 1 {
            support(agents, config, slot, start, end);
        }
    }
    for slot in start..end {
        if living(get(agents, slot, A::LIFE)) {
            consume(agents, firms, config, group, slot);
        }
    }
    if config[C::BIRTH_RULE] == 1 {
        reproduce(agents, firms, config, group, day);
    }
}
