use super::{Cell as C, Parameters as P};
use cubecl::prelude::*;

#[cube]
fn read(cells: &[u32], index: u32, field: usize) -> u32 {
    cells[index as usize * C::WORDS + field]
}

#[cube]
fn write(cells: &mut [u32], index: u32, field: usize, value: u32) {
    cells[index as usize * C::WORDS + field] = value;
}

#[cube]
fn copy(input: &[u32], output: &mut [u32], index: u32) {
    for field in 0..C::WORDS {
        write(output, index, field, read(input, index, field));
    }
}

#[cube]
fn neighbor(index: u32, direction: u32, config: &[u32]) -> u32 {
    let width = config[P::WIDTH];
    let height = config[P::HEIGHT];
    let x = index % width;
    let y = index / width;
    let west = (x + width - 1) % width;
    let east = (x + 1) % width;
    let north = (y + height - 1) % height;
    let south = (y + 1) % height;
    match direction {
        0 => north * width + west,
        1 => north * width + x,
        2 => north * width + east,
        3 => y * width + east,
        4 => south * width + east,
        5 => south * width + x,
        6 => south * width + west,
        _ => y * width + west,
    }
}

#[cube]
fn action(input: &[u32], index: u32, config: &[u32]) -> u32 {
    let mut living = 0u32;
    let mut cooperative = 0u32;
    for direction in 0..8u32 {
        let peer = neighbor(index, direction, config);
        let alive = read(input, peer, C::LIFE) == 1;
        living += u32::cast_from(alive);
        cooperative += u32::cast_from(alive && read(input, peer, C::ACTION) == 1);
    }
    match read(input, index, C::STRATEGY) {
        0 => 1u32,
        1 => 2u32,
        _ => select(2 * cooperative >= living, 1u32, 2u32),
    }
}

#[cube]
fn prepare_cell(input: &[u32], output: &mut [u32], config: &[u32], index: u32) {
    copy(input, output, index);
    for field in C::CONTRIBUTION..C::WORDS {
        write(output, index, field, 0);
    }
    let alive = read(input, index, C::LIFE) == 1;
    let available_food = min(
        config[P::FOOD_CAPACITY],
        read(input, index, C::FOOD) + config[P::REGROWTH],
    );
    let harvested = select(alive, min(available_food, config[P::HARVEST]), 0);
    let energy = read(input, index, C::ENERGY) + harvested;
    let selected = select(alive, action(input, index, config), 0);
    let mut recipients = 0u32;
    for direction in 0..8u32 {
        recipients += u32::cast_from(read(input, neighbor(index, direction, config), C::LIFE) == 1);
    }
    let contribution = select(
        selected == 1 && recipients > 0,
        min(
            config[P::COOPERATION],
            energy - min(energy, config[P::METABOLISM]),
        ),
        0,
    );
    write(output, index, C::LIFE, select(alive, 1u32, 0u32));
    write(
        output,
        index,
        C::AGE,
        read(input, index, C::AGE) + u32::cast_from(alive),
    );
    write(output, index, C::ENERGY, energy);
    write(output, index, C::FOOD, available_food - harvested);
    write(output, index, C::ACTION, selected);
    write(output, index, C::EVENT, 0);
    write(output, index, C::CONTRIBUTION, contribution);
    write(
        output,
        index,
        C::REGROWN,
        available_food - read(input, index, C::FOOD),
    );
    write(output, index, C::HARVESTED, harvested);
}

#[cube(launch)]
pub fn prepare(input: &[u32], output: &mut [u32], config: &[u32]) {
    let index = ABSOLUTE_POS as u32;
    if index < config[P::WIDTH] * config[P::HEIGHT] {
        prepare_cell(input, output, config, index);
    }
}

#[cube]
fn theft(input: &[u32], victim: u32, config: &[u32]) -> u32 {
    let mut exploiters = 0u32;
    for direction in 0..8u32 {
        let peer = neighbor(victim, direction, config);
        exploiters +=
            u32::cast_from(read(input, peer, C::LIFE) == 1 && read(input, peer, C::ACTION) == 2);
    }
    let available = read(input, victim, C::ENERGY) - read(input, victim, C::CONTRIBUTION);
    select(
        read(input, victim, C::ACTION) == 1,
        min(available, exploiters * config[P::EXPLOITATION]),
        0,
    )
}

#[cube]
fn portion(
    input: &[u32],
    donor: u32,
    recipient: u32,
    kind: u32,
    amount: u32,
    config: &[u32],
    tick: u32,
) -> u32 {
    let mut rank = 0u32;
    let mut own_rank = 0u32;
    let mut receives = false;
    for offset in 0..8u32 {
        let peer = neighbor(donor, (offset + tick % 8) % 8, config);
        let eligible =
            read(input, peer, C::LIFE) == 1 && (kind == 0 || read(input, peer, C::ACTION) == 2);
        own_rank = select(peer == recipient, rank, own_rank);
        receives = receives || (peer == recipient && eligible);
        rank += u32::cast_from(eligible);
    }
    let divisor = max(rank, 1u32);
    select(
        receives,
        amount / divisor + u32::cast_from(own_rank < amount % divisor),
        0,
    )
}

#[cube]
fn interact_cell(input: &[u32], output: &mut [u32], config: &[u32], index: u32, tick: u32) {
    copy(input, output, index);
    let alive = read(input, index, C::LIFE) == 1;
    let contribution = read(input, index, C::CONTRIBUTION);
    let mut income = 0u32;
    for direction in 0..8u32 {
        let donor = neighbor(index, direction, config);
        income += portion(
            input,
            donor,
            index,
            0,
            2 * read(input, donor, C::CONTRIBUTION),
            config,
            tick,
        );
        income += portion(
            input,
            donor,
            index,
            1,
            theft(input, donor, config),
            config,
            tick,
        );
    }
    let available =
        read(input, index, C::ENERGY) - contribution - theft(input, index, config) + income;
    let cost = select(
        alive,
        config[P::METABOLISM]
            + u32::cast_from(read(input, index, C::ACTION) == 2 && config[P::EXPLOITATION] > 0),
        0,
    );
    let spent = min(available, cost);
    let hungry = alive && available < cost;
    let died = alive && (hungry || read(input, index, C::AGE) >= config[P::MAXIMUM_AGE]);
    let remaining = available - spent;
    let energy = select(died, 0, remaining);
    let can_birth = alive
        && !died
        && energy >= config[P::BIRTH_THRESHOLD]
        && read(input, index, C::AGE) >= config[P::MATURITY];
    let mut claim = 0u32;
    for offset in 0..8u32 {
        let direction = (offset + read(input, index, C::ID) % 8 + tick % 8) % 8;
        let target = neighbor(index, direction, config);
        claim = select(
            claim == 0 && read(input, target, C::LIFE) == 0,
            target + 1,
            claim,
        );
    }
    write(output, index, C::ENERGY, energy);
    write(
        output,
        index,
        C::LIFE,
        select(died, 2u32, read(input, index, C::LIFE)),
    );
    write(output, index, C::EVENT, select(died, 2u32, 0u32));
    write(output, index, C::GENERATED, contribution);
    write(output, index, C::SPENT, spent);
    write(output, index, C::DISCARDED, select(died, remaining, 0));
    write(output, index, C::HUNGRY, u32::cast_from(hungry));
    write(output, index, C::CLAIM, select(can_birth, claim, 0));
}

#[cube(launch)]
pub fn interact(input: &[u32], output: &mut [u32], config: &[u32], tick: u32) {
    let index = ABSOLUTE_POS as u32;
    if index < config[P::WIDTH] * config[P::HEIGHT] {
        interact_cell(input, output, config, index, tick);
    }
}

#[cube]
fn winner(input: &[u32], target: u32, config: &[u32], tick: u32) -> u32 {
    let count = config[P::WIDTH] * config[P::HEIGHT];
    let mut selected = count;
    let mut priority = count;
    for direction in 0..8u32 {
        let parent = neighbor(target, direction, config);
        let rank = (parent + tick % count) % count;
        let claims = read(input, parent, C::CLAIM) == target + 1
            && read(input, parent, C::LIFE) == 1
            && rank < priority;
        selected = select(claims, parent, selected);
        priority = select(claims, rank, priority);
    }
    selected
}

#[cube]
fn mix(value: u32) -> u32 {
    let first = (value ^ (value >> 16)) * 0x7feb352du32;
    let second = (first ^ (first >> 15)) * 0x846ca68bu32;
    second ^ (second >> 16)
}

#[cube]
fn reproduce_cell(
    input: &[u32],
    output: &mut [u32],
    config: &[u32],
    index: u32,
    tick: u32,
    epoch: u32,
) {
    copy(input, output, index);
    let count = config[P::WIDTH] * config[P::HEIGHT];
    let claim = read(input, index, C::CLAIM);
    let success = claim > 0 && winner(input, max(claim, 1u32) - 1, config, tick) == index;
    let energy = read(input, index, C::ENERGY) - select(success, config[P::CHILD_ENERGY], 0);
    write(output, index, C::ENERGY, energy);
    let selected = winner(input, index, config, tick);
    let born = read(input, index, C::LIFE) == 0 && selected < count;
    if born {
        let parent = min(selected, count - 1);
        let inherited = read(input, parent, C::STRATEGY);
        let random = mix(config[P::SEED] ^ read(input, parent, C::ID) ^ mix(tick) ^ mix(index));
        let changed = (inherited + 1 + (random >> 16) % 2) % 3;
        let strategy = select(random % 1000 < config[P::MUTATION], changed, inherited);
        write(output, index, C::ID, epoch * count + index + 1);
        write(output, index, C::PARENT, read(input, parent, C::ID));
        write(output, index, C::STRATEGY, strategy);
        write(output, index, C::AGE, 0);
        write(output, index, C::ENERGY, config[P::CHILD_ENERGY]);
        write(output, index, C::LIFE, 1);
        write(output, index, C::ACTION, 0);
        write(output, index, C::EVENT, 1);
        write(output, index, C::CLAIM, 0);
    }
}

#[cube(launch)]
pub fn reproduce(input: &[u32], output: &mut [u32], config: &[u32], tick: u32, epoch: u32) {
    let index = ABSOLUTE_POS as u32;
    if index < config[P::WIDTH] * config[P::HEIGHT] {
        reproduce_cell(input, output, config, index, tick, epoch);
    }
}
