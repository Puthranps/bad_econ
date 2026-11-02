use bytemuck::{Pod, Zeroable};
use std::mem::offset_of;

macro_rules! layout {
    ($name:ident { $($field:ident: $constant:ident),* $(,)? }) => {
        #[repr(C)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        pub(super) struct $name { $(pub $field: u32),* }
        impl $name {
            pub const WORDS: usize = size_of::<Self>() / 4;
            $(pub const $constant: usize = offset_of!(Self, $field) / 4;)*
        }
    };
}

layout!(Agent {
    identity: ID,
    parent: PARENT,
    strategy: STRATEGY,
    trade: TRADE,
    life: LIFE,
    age: AGE,
    food: FOOD,
    cash: CASH,
    hunger: HUNGER,
    cooldown: COOLDOWN,
    last_fraud: LAST_FRAUD,
    action: ACTION,
    contributed: CONTRIBUTED,
    private: PRIVATE,
    shared: SHARED,
    wage: WAGE,
    reservation: RESERVATION,
    employed: EMPLOYED,
    shortfall: SHORTFALL,
    bought: BOUGHT,
    sold: SOLD,
    cheated: CHEATED,
    refused: REFUSED,
    frauds: FRAUDS,
    consumed: CONSUMED,
});

layout!(Firm {
    cash: CASH,
    food: FOOD,
    wage: WAGE,
    workers: WORKERS,
    produced: PRODUCED,
    births: BIRTHS,
    deaths: DEATHS,
    trades: TRADES,
    frauds: FRAUDS,
    abandoned: ABANDONED,
});

layout!(Settings {
    capacity: CAPACITY,
    size: SIZE,
    effort: EFFORT,
    project: PROJECT,
    productivity: PRODUCTIVITY,
    need: NEED,
    child_need: CHILD_NEED,
    threshold: THRESHOLD,
    sharing: SHARING,
    trade_rule: TRADE_RULE,
    price: PRICE,
    jobs: JOBS,
    work: WORK,
    work_food: WORK_FOOD,
    maximum_wage: MAXIMUM_WAGE,
    reservation: RESERVATION,
    wage_rule: WAGE_RULE,
    adult_age: ADULT_AGE,
    starvation: STARVATION,
    birth_rule: BIRTH_RULE,
    birth_food: BIRTH_FOOD,
    birth_interval: BIRTH_INTERVAL,
});
