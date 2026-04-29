/// Hand categories from worst (`HighCard`) to best (`StraightFlush`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HandCategory {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

/// Returns the category encoded in the top 4 bits of the 16-bit rank.
#[inline]
pub fn get_hand_category(hand_rank: u16) -> HandCategory {
    match hand_rank >> 12 {
        0 => HandCategory::HighCard,
        1 => HandCategory::OnePair,
        2 => HandCategory::TwoPair,
        3 => HandCategory::ThreeOfAKind,
        4 => HandCategory::Straight,
        5 => HandCategory::Flush,
        6 => HandCategory::FullHouse,
        7 => HandCategory::FourOfAKind,
        8 => HandCategory::StraightFlush,
        _ => unreachable!(),
    }
}
