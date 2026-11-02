use super::{
    kernels::*,
    layout::{Agent as A, Firm as F, Settings as C},
};
use cubecl::prelude::*;

#[cube]
fn peer_trade(
    agents: &mut [u32],
    firms: &mut [u32],
    config: &[u32],
    group: u32,
    buyer: u32,
    seller: u32,
) {
    let price = config[C::PRICE];
    let deficit =
        need(agents, buyer, config) - min(need(agents, buyer, config), get(agents, buyer, A::FOOD));
    let reserve = need(agents, seller, config);
    let surplus = get(agents, seller, A::FOOD) - min(reserve, get(agents, seller, A::FOOD));
    let quantity = min(deficit, min(surplus, get(agents, buyer, A::CASH) / price));
    let dishonest = get(agents, seller, A::TRADE) == 1 && config[C::TRADE_RULE] == 0;
    let delivered = select(dishonest, 0, quantity);
    let cost = quantity * price;
    let buyer_cash = get(agents, buyer, A::CASH) - cost;
    let seller_food = get(agents, seller, A::FOOD) - delivered;
    put(agents, buyer, A::CASH, buyer_cash);
    put(agents, seller, A::FOOD, seller_food);
    increase(agents, seller, A::CASH, cost);
    increase(agents, buyer, A::FOOD, delivered);
    increase(agents, buyer, A::BOUGHT, delivered);
    increase(agents, seller, A::SOLD, delivered);
    let fraud = u32::cast_from(dishonest && quantity > 0);
    increase(agents, buyer, A::CHEATED, fraud);
    increase(agents, seller, A::FRAUDS, fraud);
    let remembered = select(
        fraud > 0,
        get(agents, seller, A::ID),
        get(agents, buyer, A::LAST_FRAUD),
    );
    put(agents, buyer, A::LAST_FRAUD, remembered);
    let base = group as usize * F::WORDS;
    firms[base + F::TRADES] += u32::cast_from(quantity > 0);
    firms[base + F::FRAUDS] += fraud;
}

#[cube]
fn purchase(
    agents: &mut [u32],
    firms: &mut [u32],
    config: &[u32],
    group: u32,
    buyer: u32,
    day: u32,
) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    let mut seller = config[C::CAPACITY];
    for offset in 0..end - start {
        let peer = start + (offset + day) % (end - start);
        let available = get(agents, peer, A::FOOD) > need(agents, peer, config);
        let candidate = peer != buyer && living(get(agents, peer, A::LIFE)) && available;
        let refused = candidate && get(agents, peer, A::ID) == get(agents, buyer, A::LAST_FRAUD);
        increase(agents, buyer, A::REFUSED, u32::cast_from(refused));
        seller = select(
            candidate && !refused && seller == config[C::CAPACITY],
            peer,
            seller,
        );
    }
    if seller < config[C::CAPACITY] {
        peer_trade(agents, firms, config, group, buyer, seller);
    }
    let base = group as usize * F::WORDS;
    let deficit =
        need(agents, buyer, config) - min(need(agents, buyer, config), get(agents, buyer, A::FOOD));
    let quantity = min(
        deficit,
        min(
            firms[base + F::FOOD],
            get(agents, buyer, A::CASH) / config[C::PRICE],
        ),
    );
    let cost = quantity * config[C::PRICE];
    let cash = get(agents, buyer, A::CASH) - cost;
    put(agents, buyer, A::CASH, cash);
    increase(agents, buyer, A::FOOD, quantity);
    increase(agents, buyer, A::BOUGHT, quantity);
    firms[base + F::FOOD] -= quantity;
    firms[base + F::CASH] += cost;
    firms[base + F::TRADES] += u32::cast_from(quantity > 0);
}

#[cube]
pub fn run(agents: &mut [u32], firms: &mut [u32], config: &[u32], group: u32, day: u32) {
    let start = group * config[C::SIZE];
    let end = min(start + config[C::SIZE], config[C::CAPACITY]);
    for offset in 0..end - start {
        let buyer = start + (offset + day) % (end - start);
        let shopping = living(get(agents, buyer, A::LIFE))
            && get(agents, buyer, A::FOOD) < need(agents, buyer, config);
        if shopping {
            purchase(agents, firms, config, group, buyer, day);
        }
    }
}
