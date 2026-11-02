use super::{
    kernels::*,
    layout::{Agent as A, Settings as C},
};
use cubecl::prelude::*;

#[cube]
fn choice(previous: &[u32], slot: u32, start: u32, end: u32, config: &[u32]) -> u32 {
    let mut peers = 0u32;
    let mut contributors = 0u32;
    let mut best = read(previous, slot, A::PRIVATE)
        + read(previous, slot, A::SHARED)
        + read(previous, slot, A::WAGE) / config[C::PRICE];
    let mut imitation = max(1u32, read(previous, slot, A::ACTION));
    for peer in start..end {
        let observed = peer != slot && read(previous, peer, A::LIFE) == 2;
        peers += u32::cast_from(observed);
        contributors += u32::cast_from(observed && read(previous, peer, A::ACTION) == 2);
        let income = read(previous, peer, A::PRIVATE)
            + read(previous, peer, A::SHARED)
            + read(previous, peer, A::WAGE) / config[C::PRICE];
        let better = observed && income > best;
        imitation = select(
            better,
            max(1u32, read(previous, peer, A::ACTION)),
            imitation,
        );
        best = select(better, income, best);
    }
    match read(previous, slot, A::STRATEGY) {
        0 => 2u32,
        1 => 1u32,
        2 => select(
            contributors * 100 >= config[C::THRESHOLD] * peers,
            2u32,
            1u32,
        ),
        _ => imitation,
    }
}

#[cube]
pub fn run(previous: &[u32], agents: &mut [u32], config: &[u32], group: u32, day: u32) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    let mut contributors = 0u32;
    let mut recipients = 0u32;
    for slot in start..end {
        let action = select(
            get(agents, slot, A::LIFE) == 2,
            choice(previous, slot, start, end, config),
            0,
        );
        put(agents, slot, A::ACTION, action);
        contributors += u32::cast_from(action == 2);
        recipients += u32::cast_from(
            living(get(agents, slot, A::LIFE)) && (config[C::SHARING] == 0 || action == 2),
        );
    }
    let output = contributors * config[C::PROJECT] * config[C::PRODUCTIVITY];
    let divisor = max(1, recipients);
    let mut rank = 0u32;
    for offset in 0..end - start {
        let slot = start + (offset + day) % (end - start);
        let action = get(agents, slot, A::ACTION);
        let contribution = select(action == 2, config[C::PROJECT], 0);
        let private = select(
            get(agents, slot, A::LIFE) == 2,
            config[C::EFFORT] - contribution - get(agents, slot, A::EMPLOYED) * config[C::WORK],
            0,
        );
        let eligible =
            living(get(agents, slot, A::LIFE)) && (config[C::SHARING] == 0 || action == 2);
        let shared = select(
            eligible,
            output / divisor + u32::cast_from(rank < output % divisor),
            0,
        );
        rank += u32::cast_from(eligible);
        put(agents, slot, A::CONTRIBUTED, contribution);
        put(agents, slot, A::PRIVATE, private);
        put(agents, slot, A::SHARED, shared);
        increase(agents, slot, A::FOOD, private + shared);
    }
}
