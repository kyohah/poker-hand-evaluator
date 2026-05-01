/// Hand categories from worst (`HighCard`) to best (`StraightFlush`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HandCategory {
    /// No pair — five cards of mixed ranks, no flush, no straight.
    HighCard = 0,
    /// One pair plus three kickers.
    OnePair = 1,
    /// Two pair plus a kicker.
    TwoPair = 2,
    /// Three of a kind plus two kickers.
    ThreeOfAKind = 3,
    /// Five cards of consecutive ranks.
    Straight = 4,
    /// Five cards of the same suit, not consecutive.
    Flush = 5,
    /// Three of a kind plus a pair.
    FullHouse = 6,
    /// Four of a kind plus a kicker.
    FourOfAKind = 7,
    /// Straight all of one suit (ace-high is a "royal").
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
