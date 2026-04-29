use phe_core::Hand;

/// Parses a Hold'em hand string like `"AsKsQsJsTs"`.
///
/// Rank characters: `2-9 T J Q K A` (Ace-high). Suit characters: `c d h s`,
/// case-insensitive on both. Each card is exactly two characters; an empty
/// input returns an empty hand.
pub fn parse_hand(hand_str: &str) -> Result<Hand, String> {
    let mut hand = Hand::new();
    let mut chars = hand_str.chars();
    loop {
        let rank_char = match chars.next() {
            Some(c) => c,
            None => return Ok(hand),
        };
        let suit_char = chars
            .next()
            .ok_or("parse failed: expected suit character, but got EOF")?;
        let rank_id = match rank_char.to_ascii_uppercase() {
            '2' => 0,
            '3' => 1,
            '4' => 2,
            '5' => 3,
            '6' => 4,
            '7' => 5,
            '8' => 6,
            '9' => 7,
            'T' => 8,
            'J' => 9,
            'Q' => 10,
            'K' => 11,
            'A' => 12,
            ch => {
                return Err(format!(
                    "parse failed: expected rank character, but got '{}'",
                    ch
                ))
            }
        };
        let suit_id = match suit_char.to_ascii_lowercase() {
            'c' => 0,
            'd' => 1,
            'h' => 2,
            's' => 3,
            ch => {
                return Err(format!(
                    "parse failed: expected suit character, but got '{}'",
                    ch
                ))
            }
        };
        hand = hand.add_card(rank_id * 4 + suit_id);
    }
}
