use crate::eval::HighRule;
use phe_core::{Hand, CARDS, NUMBER_OF_CARDS};
use phe_holdem_assets::HEADS_UP_WIN_FREQUENCY;

/// Returns `(hand1 wins, hand2 wins, ties)` over every legal completion of
/// the supplied state. `hand1` must be 2 cards; `hand2` may be 0, 1, or 2;
/// `board` may be 0/3/4/5; `dead_cards` may be any size.
pub fn heads_up_win_frequency(
    hand1: &Hand,
    hand2: &Hand,
    board: &Hand,
    dead_cards: &Hand,
) -> (u32, u32, u32) {
    assert_eq!(hand1.len(), 2);
    assert!(hand2.len() <= 2);
    assert!(matches!(board.len(), 0 | 3 | 4 | 5));
    assert_eq!(
        (*hand1 + *hand2 + *board + *dead_cards).len(),
        hand1.len() + hand2.len() + board.len() + dead_cards.len()
    );
    let alive = compute_alive_cards(
        hand1.get_mask() | hand2.get_mask() | board.get_mask() | dead_cards.get_mask(),
    );
    assert!(alive.len() >= 5 - board.len());
    let h1 = *hand1 + *board;
    let h2 = *hand2 + *board;
    match (hand2.len() - board.len(), board.len()) {
        (0, 0) => match dead_cards.len() {
            0 => freq_0_0(&h1),
            _ => freq_runout_0(&h1, &h2, &alive, freq_2_0),
        },
        (0, 3) => freq_runout_0(&h1, &h2, &alive, freq_2_3),
        (0, 4) => freq_runout_0(&h1, &h2, &alive, freq_2_4),
        (0, 5) => freq_runout_0(&h1, &h2, &alive, freq_2_5),
        (1, 0) => freq_runout_1(&h1, &h2, &alive, freq_2_0),
        (1, 3) => freq_runout_1(&h1, &h2, &alive, freq_2_3),
        (1, 4) => freq_runout_1(&h1, &h2, &alive, freq_2_4),
        (1, 5) => freq_runout_1(&h1, &h2, &alive, freq_2_5),
        (2, 0) => freq_2_0(&h1, &h2, &alive),
        (2, 3) => freq_2_3(&h1, &h2, &alive),
        (2, 4) => freq_2_4(&h1, &h2, &alive),
        (2, 5) => freq_2_5(&h1, &h2, &alive),
        _ => unreachable!(),
    }
}

fn compute_alive_cards(mask: u64) -> Vec<usize> {
    (0..NUMBER_OF_CARDS)
        .filter(|&i| (CARDS[i].1 & mask) == 0)
        .collect()
}

fn freq_0_0(hand: &Hand) -> (u32, u32, u32) {
    let cards: Vec<usize> = (0..NUMBER_OF_CARDS)
        .filter(|&i| (CARDS[i].1 & hand.get_mask()) != 0)
        .collect();
    let rank1 = cards[0] / 4;
    let suit1 = cards[0] % 4;
    let rank2 = cards[1] / 4;
    let suit2 = cards[1] % 4;
    if suit1 == suit2 {
        HEADS_UP_WIN_FREQUENCY[rank1 * 13 + rank2]
    } else {
        HEADS_UP_WIN_FREQUENCY[rank2 * 13 + rank1]
    }
}

fn freq_runout_0(
    hand1: &Hand,
    hand2: &Hand,
    alive: &[usize],
    inner: fn(&Hand, &Hand, &[usize]) -> (u32, u32, u32),
) -> (u32, u32, u32) {
    let len = alive.len();
    let mut acc = (0, 0, 0);
    for i in 0..(len - 1) {
        let h2 = hand2.add_card(alive[i]);
        for j in (i + 1)..len {
            let h2 = h2.add_card(alive[j]);
            let rest: Vec<usize> = alive
                .iter()
                .enumerate()
                .filter_map(|(idx, x)| (idx != i && idx != j).then_some(*x))
                .collect();
            let r = inner(hand1, &h2, &rest);
            acc.0 += r.0;
            acc.1 += r.1;
            acc.2 += r.2;
        }
    }
    acc
}

fn freq_runout_1(
    hand1: &Hand,
    hand2: &Hand,
    alive: &[usize],
    inner: fn(&Hand, &Hand, &[usize]) -> (u32, u32, u32),
) -> (u32, u32, u32) {
    let mut acc = (0, 0, 0);
    for (i, &c) in alive.iter().enumerate() {
        let h2 = hand2.add_card(c);
        let rest: Vec<usize> = alive
            .iter()
            .enumerate()
            .filter_map(|(idx, x)| (idx != i).then_some(*x))
            .collect();
        let r = inner(hand1, &h2, &rest);
        acc.0 += r.0;
        acc.1 += r.1;
        acc.2 += r.2;
    }
    acc
}

fn tally(rank1: u16, rank2: u16, count: &mut (u32, u32, u32)) {
    if rank1 > rank2 {
        count.0 += 1;
    } else if rank1 < rank2 {
        count.1 += 1;
    } else {
        count.2 += 1;
    }
}

fn freq_2_0(hand1: &Hand, hand2: &Hand, alive: &[usize]) -> (u32, u32, u32) {
    let len = alive.len();
    let mut count = (0, 0, 0);
    for i in 0..(len - 4) {
        let h1 = hand1.add_card(alive[i]);
        let h2 = hand2.add_card(alive[i]);
        for j in (i + 1)..(len - 3) {
            let h1 = h1.add_card(alive[j]);
            let h2 = h2.add_card(alive[j]);
            for k in (j + 1)..(len - 2) {
                let h1 = h1.add_card(alive[k]);
                let h2 = h2.add_card(alive[k]);
                for m in (k + 1)..(len - 1) {
                    let h1 = h1.add_card(alive[m]);
                    let h2 = h2.add_card(alive[m]);
                    for n in (m + 1)..len {
                        let h1 = h1.add_card(alive[n]);
                        let h2 = h2.add_card(alive[n]);
                        tally(HighRule::evaluate(&h1), HighRule::evaluate(&h2), &mut count);
                    }
                }
            }
        }
    }
    count
}

fn freq_2_3(hand1: &Hand, hand2: &Hand, alive: &[usize]) -> (u32, u32, u32) {
    let len = alive.len();
    let mut count = (0, 0, 0);
    for i in 0..(len - 1) {
        let h1 = hand1.add_card(alive[i]);
        let h2 = hand2.add_card(alive[i]);
        for j in (i + 1)..len {
            let h1 = h1.add_card(alive[j]);
            let h2 = h2.add_card(alive[j]);
            tally(HighRule::evaluate(&h1), HighRule::evaluate(&h2), &mut count);
        }
    }
    count
}

fn freq_2_4(hand1: &Hand, hand2: &Hand, alive: &[usize]) -> (u32, u32, u32) {
    let mut count = (0, 0, 0);
    for &c in alive {
        let h1 = hand1.add_card(c);
        let h2 = hand2.add_card(c);
        tally(HighRule::evaluate(&h1), HighRule::evaluate(&h2), &mut count);
    }
    count
}

fn freq_2_5(hand1: &Hand, hand2: &Hand, _: &[usize]) -> (u32, u32, u32) {
    let r1 = HighRule::evaluate(hand1);
    let r2 = HighRule::evaluate(hand2);
    if r1 > r2 {
        (1, 0, 0)
    } else if r1 < r2 {
        (0, 1, 0)
    } else {
        (0, 0, 1)
    }
}
