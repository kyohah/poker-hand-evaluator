/// Number of ranks (A, 2, 3, ..., K)
pub const NUMBER_OF_RANKS: usize = 13;

/// Number of cards in a standard deck
pub const NUMBER_OF_CARDS: usize = 4 * NUMBER_OF_RANKS;

/// Determines perfect hash function. Adjust this parameter to modify the offset table.
pub const OFFSET_SHIFT: usize = 11;

/// Rank keys that guarantee a unique sum for every rank combination of 5-7 cards.
/// Reused from holdem-hand-evaluator (the mathematical property is rank-ordering agnostic).
/// Index mapping for 8-low: 0=A(low), 1=2, 2=3, 3=4, 4=5, 5=6, 6=7, 7=8, 8=9, 9=T, 10=J, 11=Q, 12=K
pub const RANK_BASES: [u64; NUMBER_OF_RANKS] = [
    0x000800, 0x002000, 0x024800, 0x025005, 0x03102e, 0x05f0e4, 0x13dc93, 0x344211, 0x35a068,
    0x377813, 0x378001, 0x378800, 0x380000,
];

/// Max rank key value (4 kings + 3 queens)
pub const MAX_RANK_KEY: u64 =
    4 * RANK_BASES[NUMBER_OF_RANKS - 1] + 3 * RANK_BASES[NUMBER_OF_RANKS - 2];

/// Rank names for display (index 0 = A, index 12 = K)
pub const RANK_CHARS: [char; NUMBER_OF_RANKS] = [
    'A', '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K',
];

/// (card key, bit mask) of cards
/// Card ID: id = rank * 4 + suit
/// Rank: 0=A(low), 1=2, 2=3, ..., 12=K
/// Suit: 0=club, 1=diamond, 2=heart, 3=spade
/// Key contains only rank information (no suit data needed for low evaluation).
/// Mask uses a simple scheme: one bit per card (bit position = card ID).
#[rustfmt::skip]
pub const CARDS: [(u64, u64); NUMBER_OF_CARDS] = [
    /* Ac */ (RANK_BASES[0],  1 << 0),
    /* Ad */ (RANK_BASES[0],  1 << 1),
    /* Ah */ (RANK_BASES[0],  1 << 2),
    /* As */ (RANK_BASES[0],  1 << 3),
    /* 2c */ (RANK_BASES[1],  1 << 4),
    /* 2d */ (RANK_BASES[1],  1 << 5),
    /* 2h */ (RANK_BASES[1],  1 << 6),
    /* 2s */ (RANK_BASES[1],  1 << 7),
    /* 3c */ (RANK_BASES[2],  1 << 8),
    /* 3d */ (RANK_BASES[2],  1 << 9),
    /* 3h */ (RANK_BASES[2],  1 << 10),
    /* 3s */ (RANK_BASES[2],  1 << 11),
    /* 4c */ (RANK_BASES[3],  1 << 12),
    /* 4d */ (RANK_BASES[3],  1 << 13),
    /* 4h */ (RANK_BASES[3],  1 << 14),
    /* 4s */ (RANK_BASES[3],  1 << 15),
    /* 5c */ (RANK_BASES[4],  1 << 16),
    /* 5d */ (RANK_BASES[4],  1 << 17),
    /* 5h */ (RANK_BASES[4],  1 << 18),
    /* 5s */ (RANK_BASES[4],  1 << 19),
    /* 6c */ (RANK_BASES[5],  1 << 20),
    /* 6d */ (RANK_BASES[5],  1 << 21),
    /* 6h */ (RANK_BASES[5],  1 << 22),
    /* 6s */ (RANK_BASES[5],  1 << 23),
    /* 7c */ (RANK_BASES[6],  1 << 24),
    /* 7d */ (RANK_BASES[6],  1 << 25),
    /* 7h */ (RANK_BASES[6],  1 << 26),
    /* 7s */ (RANK_BASES[6],  1 << 27),
    /* 8c */ (RANK_BASES[7],  1 << 28),
    /* 8d */ (RANK_BASES[7],  1 << 29),
    /* 8h */ (RANK_BASES[7],  1 << 30),
    /* 8s */ (RANK_BASES[7],  1 << 31),
    /* 9c */ (RANK_BASES[8],  1 << 32),
    /* 9d */ (RANK_BASES[8],  1 << 33),
    /* 9h */ (RANK_BASES[8],  1 << 34),
    /* 9s */ (RANK_BASES[8],  1 << 35),
    /* Tc */ (RANK_BASES[9],  1 << 36),
    /* Td */ (RANK_BASES[9],  1 << 37),
    /* Th */ (RANK_BASES[9],  1 << 38),
    /* Ts */ (RANK_BASES[9],  1 << 39),
    /* Jc */ (RANK_BASES[10], 1 << 40),
    /* Jd */ (RANK_BASES[10], 1 << 41),
    /* Jh */ (RANK_BASES[10], 1 << 42),
    /* Js */ (RANK_BASES[10], 1 << 43),
    /* Qc */ (RANK_BASES[11], 1 << 44),
    /* Qd */ (RANK_BASES[11], 1 << 45),
    /* Qh */ (RANK_BASES[11], 1 << 46),
    /* Qs */ (RANK_BASES[11], 1 << 47),
    /* Kc */ (RANK_BASES[12], 1 << 48),
    /* Kd */ (RANK_BASES[12], 1 << 49),
    /* Kh */ (RANK_BASES[12], 1 << 50),
    /* Ks */ (RANK_BASES[12], 1 << 51),
];
