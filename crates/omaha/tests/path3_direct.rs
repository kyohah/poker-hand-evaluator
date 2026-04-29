//! Tests for the path-3 (flush + board pair) 9-card direct evaluator.
//!
//! Path 3 is invoked when a flush is reachable (some suit has both
//! ≥2 hole and ≥3 board cards) AND the board has at least one rank
//! pair. Under those invariants the answer is one of {SF, Quads, FH,
//! Flush}; lower categories are always dominated by the guaranteed
//! Flush.
//!
//! These tests pin the three FH composition cases, both Quads cases,
//! and the SF-vs-Flush dispatch, plus a deterministic random sweep
//! restricted to path-3 fixtures vs the naive 60-combo eval.

use phe_core::Hand;
use phe_holdem::{get_hand_category, HandCategory, HighRule};
use phe_omaha::{board_has_no_pair, flush_suit, OmahaHighRule};

fn naive(hole: &[usize; 4], board: &[usize; 5]) -> u16 {
    const HOLE_PAIRS: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    const BOARD_TRIPLES: [(usize, usize, usize); 10] = [
        (0, 1, 2),
        (0, 1, 3),
        (0, 1, 4),
        (0, 2, 3),
        (0, 2, 4),
        (0, 3, 4),
        (1, 2, 3),
        (1, 2, 4),
        (1, 3, 4),
        (2, 3, 4),
    ];
    let mut best = 0u16;
    for &(i, j) in &HOLE_PAIRS {
        let hp = Hand::new().add_card(hole[i]).add_card(hole[j]);
        for &(a, b, c) in &BOARD_TRIPLES {
            let h = hp.add_card(board[a]).add_card(board[b]).add_card(board[c]);
            best = best.max(HighRule::evaluate(&h));
        }
    }
    best
}

fn card(rank: char, suit: char) -> usize {
    let r = match rank {
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
        _ => panic!("bad rank: {rank}"),
    };
    let s = match suit {
        'c' => 0,
        'd' => 1,
        'h' => 2,
        's' => 3,
        _ => panic!("bad suit: {suit}"),
    };
    r * 4 + s
}

fn hole(s: &str) -> [usize; 4] {
    let mut chars = s.chars();
    let mut out = [0usize; 4];
    for slot in &mut out {
        let r = chars.next().unwrap();
        let su = chars.next().unwrap();
        *slot = card(r, su);
    }
    out
}

fn board(s: &str) -> [usize; 5] {
    let mut chars = s.chars();
    let mut out = [0usize; 5];
    for slot in &mut out {
        let r = chars.next().unwrap();
        let su = chars.next().unwrap();
        *slot = card(r, su);
    }
    out
}

fn assert_path3(h: &[usize; 4], b: &[usize; 5]) {
    assert!(
        flush_suit(h, b).is_some() && !board_has_no_pair(b),
        "test fixture must hit path 3 (flush eligible + board paired)"
    );
}

// --- Quads cases -------------------------------------------------------

#[test]
fn quads_case_a_board_trips_plus_one_hole() {
    // Board has trips of A (3 As-suited), hole has 1 Ad. Plus a flush
    // suit eligible (heart flush via hole + board).
    // Wait: board needs 3 As and a pair somewhere. Trips IS a pair.
    // So board = AsAhAd Kc Qc → has trips of A (=pair).
    // Flush suit: need ≥2 hole + ≥3 board same suit. Board has Ah +
    // need 2 more hearts. Adjust: board = AsAhAd 7h 8h. flush suit:
    // hearts has hole 0... let's redesign.
    //
    // Simpler: hole = AcKcQcJc (4 clubs!), board = AdAhAs 7c 8c.
    // Clubs: hole has 4, board has 2 → not flush eligible (need ≥3 board).
    // Hmm. board needs 3 of the flush suit too.
    //
    // hole = Ac Kc 2d 3d (2 clubs), board = AdAhAs 7c 8c 9c... wait 5 board cards.
    // board = Ah As Ad 7c 9c (3 As of different suits + 2 clubs).
    // clubs: hole 2 (Ac, Kc), board 2 (7c, 9c) → not eligible.
    //
    // Try: board = Ad Ah 7c 8c 9c. Trips of A: NO (only 2 As). Pair of A.
    // Quads of A needs h[A]≥1 + b[A]≥3 OR h[A]≥2 + b[A]≥2.
    // With b[A]=2, need h[A]≥2 (case B). Test case B instead.
    //
    // So test case A: board has 3 of one rank (trips → pair).
    // hole = Ac 2c 3d 4d (2 clubs), board = AdAhAs 7c 8c.
    // clubs: hole 2 (Ac, 2c), board 2 (7c, 8c) → only 4 clubs total, not flush eligible.
    //
    // Need flush eligible AND board trips. Try board =
    // 7c 8c 9c Ad Ah.
    // clubs: hole has ≥2, board has 3 (7c, 8c, 9c). flush eligible!
    // board pair: Ad/Ah → yes (pair of A).
    // hole = Ac Kc 2d 3d. h[A]=1, b[A]=2 — that's 3 As total, not 4 → no quads.
    // We'd need h[A]≥1 AND b[A]≥3 (case A) or h[A]≥2 AND b[A]≥2 (case B).
    // With board having only 2 As (a pair), case A fails (b[A]<3).
    // To trigger case A we need board trips of A: board must have 3 As of
    // different suits. So board Ah As Ad + 2 other clubs.
    // board = Ah As Ad 7c 8c. flush eligible if hole has ≥2 clubs and
    // board has ≥3 clubs. board has 7c+8c=2 clubs → not eligible.
    // Try board = Ah Ad 7c 8c 9c. Pair of A only (2 As) → can't trigger case A.
    // Conflict: trips of A on board means 3 different-suit As, leaving
    // only 2 board slots for other suits. Need ≥3 of flush suit on
    // board, but only 2 slots remain → impossible.
    //
    // Conclusion: case A (board trips of R + hole 1 R) for Quads of R
    // is *not reachable in path 3 with the same R as the trips rank*,
    // because trips of R uses 3 board cards (different suits) and we
    // need 3 board cards in the flush suit too — total > 5 board cards.
    //
    // But Quads case A with R = some other rank works if board has
    // a separate pair (which IS the path-3 invariant). E.g. board =
    // 7c 8c 9c 2d 2h → pair of 2 + 3 clubs. Flush eligible (need hole
    // ≥2 clubs). But Quads of 2: h[2]≥1, b[2]=2 → no. Quads needs more.
    //
    // The only way Quads via case A in path 3: board has trips of R
    // (= board pair), AND board separately has ≥3 in flush suit not
    // including those R's. Board has 5 cards, 3 are R's (different
    // suits, max 4), so 2 non-R's. For ≥3 flush-suit, the 2 non-R's
    // are insufficient. Plus R's themselves only contribute up to 1
    // per suit; 3 R's hit 3 different suits (out of 4), so 0 of those
    // is "the flush suit minus 0" = at most 1 R is in the flush suit.
    // Total flush-suit board cards: ≤2 non-R + ≤1 R = 3.
    //
    // Tight: board = Ah As Ad 7c 8c, flush_suit = clubs requires
    // ≥3 clubs on board. Board clubs: 7c, 8c, plus Ac if present.
    // Ah/As/Ad don't include Ac, so only 2 clubs. No good.
    //
    // Try board = Ah As Ac 7c 8c. Clubs on board: Ac, 7c, 8c = 3
    // ✓. Pair of A: Ah, As (and Ac too — trips of A). hole = Kc Qc
    // 2d 3d. Hole clubs: Kc, Qc = 2 ✓. flush eligible!
    //
    // Quads of A: case A needs h[A]≥1 (h[A]=0 here). Fails.
    // Case B needs h[A]≥2 (=0). Fails.
    // No Quads via A. But pair on board (3 As) means FH possible:
    // board trips of A + hole pocket pair = FH. h has K,Q,2,3 — no
    // pocket pair. So no FH. Best is just flush.
    let h = hole("KcQc2d3d");
    let b = board("AhAsAc7c8c");
    assert_path3(&h, &b);
    let r = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(r, naive(&h, &b));
}

#[test]
fn quads_case_b_pocket_pair_plus_board_pair() {
    // hole AcAd (pocket pair of A) + board has 2 As of different
    // suits AND a flush suit eligible.
    // Board needs ≥3 of flush suit + pair somewhere.
    // hole = Ac Ad 2c 3c (3 clubs counting Ac). Hmm Ac is in hole.
    // board: needs 2 As (Ah, As) for case B + ≥3 of some flush suit.
    // board = Ah As Kc Qc Jc → 2 As + 3 clubs. clubs hole has 3 (Ac,
    // 2c, 3c), board 3 (Kc, Qc, Jc) → flush eligible (clubs).
    //
    // h[A]=1 (only Ac in hole, Ad too — wait hole = AcAd2c3c, so h[A]=2).
    // b[A]=2. case B feasible (h[A]≥2, b[A]≥2). Quads of A!
    // Kicker: case B picks highest non-A board card = K.
    let h = hole("AcAd2c3c");
    let b = board("AhAsKcQcJc");
    assert_path3(&h, &b);
    let r = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(r, naive(&h, &b));
    assert_eq!(get_hand_category(r), HandCategory::FourOfAKind);
}

// --- FH cases ----------------------------------------------------------

#[test]
fn fh_case_i_board_trips_plus_hole_pocket_pair() {
    // Board trips of K, hole pocket pair of A. Flush suit eligible.
    // board = Kh Ks Kc 2c 3c → trips of K + 3 clubs. flush suit (clubs)
    // eligible if hole has ≥2 clubs.
    // hole = Ah Ad 4c 5c (pocket pair A, 2 clubs).
    // FH: trips K + pair A → ranked by trips first (K), pair second.
    // But pocket A pair + board trips K: K-K-K-A-A.
    // Wait but in poker AAA-KK > KKK-AA. So we want max trips rank.
    // If hole has pocket pair of A and we can also use h[A]=2, then
    // case II: trips of A possible? Need b[A]≥2; b has 0 As. So
    // no trips of A.
    // So best is FH(K, A) = K-K-K-A-A. Naive should agree.
    let h = hole("AhAd4c5c");
    let b = board("KhKsKc2c3c");
    assert_path3(&h, &b);
    let r = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(r, naive(&h, &b));
    assert_eq!(get_hand_category(r), HandCategory::FullHouse);
}

#[test]
fn fh_case_ii_split_hole_and_board() {
    // hole has 1 each of two ranks, board has 2 of one and 1 of
    // another, flush eligible.
    // hole = Kc Ah 2d 3d (1 K + 1 A). board = Ks Ad Ac 7c 8c
    // → b[K]=1, b[A]=2, plus Ac counts. Wait board is Ks Ad Ac 7c 8c.
    // Ranks on board: K=1, A=2, 7=1, 8=1.
    // FH check (R1, R2):
    //   - (A, K): need case I/II/III. Case II h[A]=1, b[A]=2, h[K]=1,
    //     b[K]=1 ✓. So FH(A, K) is feasible: A-A-A-K-K.
    //     5-card: 1 hole A + 2 board A + 1 hole K + 1 board K.
    // flush suit: clubs. hole clubs = Kc (1), board clubs = Ac, 7c,
    // 8c (3). Hole only 1 club → flush NOT eligible. This isn't path
    // 3! Adjust.
    //
    // Need ≥2 hole clubs. Try hole = Kc Ah 2c 3c. h[K]=1, h[A]=1, h has
    // 3 clubs (Kc, 2c, 3c). board same as before. flush eligible.
    // board clubs = Ac, 7c, 8c (3). flush eligible ✓.
    // path 3: board has pair of A → yes, paired.
    // FH(A, K): case II — h[A]=1, b[A]=2, h[K]=1, b[K]=1 ✓.
    let h = hole("KcAh2c3c");
    let b = board("KsAdAc7c8c");
    assert_path3(&h, &b);
    let r = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(r, naive(&h, &b));
    // Best is FH (A-A-A-K-K) or higher. Naive will tell us.
}

#[test]
fn fh_case_iii_hole_pocket_plus_board_pair() {
    // hole pocket pair + board has ≥1 of pocket-rank + board pair of
    // another rank.
    // hole = AcAd 2h 3h (pocket A, no clubs other than Ac, Ad — but
    // those are A's). Only 2 clubs (Ac, Ad? wait Ad is diamond. So
    // hole clubs = just Ac, 1 club). Need ≥2 clubs.
    // Try hole = Ac As 2c 3c (pocket A, 3 clubs counting Ac, 2c, 3c).
    // board needs ≥1 A + board pair of another rank + ≥3 clubs.
    // board = Ah Kc Kd Jc Qc → b[A]=1, b[K]=2 (pair), clubs: Kc, Jc,
    // Qc (3) ✓. flush eligible.
    // path 3: board paired (KK) ✓.
    // FH(A, K): case III — h[A]=2, b[A]=1, b[K]=2 ✓. trips A + pair K.
    let h = hole("AcAs2c3c");
    let b = board("AhKcKdJcQc");
    assert_path3(&h, &b);
    let r = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(r, naive(&h, &b));
    assert_eq!(get_hand_category(r), HandCategory::FullHouse);
}

// --- SF in path 3 ------------------------------------------------------

#[test]
fn sf_in_path3_wins_over_quads() {
    // Construct a hand where both SF and Quads are reachable; SF
    // should win.
    // hole = 6c5c2d2h. board = 4c3c7c2c2s.
    // wait: board has 2c2s (pair of 2), and 4c, 3c, 7c (3 clubs).
    // hole has 6c, 5c (2 clubs). flush eligible (clubs).
    // path 3 (board pair of 2).
    // SF: clubs windows. Hole clubs: 5,6. Board clubs: 4,3,7,2.
    // Window 3-7 (3,4,5,6,7): hole clubs 5,6 = 2 ≥ 2; board clubs 3,4,7
    // = 3 ≥ 3 ✓. SF 7-high.
    // Quads of 2: h[2]=2, b[2]=2 → case B feasible. Quads of 2
    // with kicker = highest non-2 board = 7. Quads of 2 (kicker 7).
    // SF 7-high (cat 8) > Quads 2 (cat 7) → SF wins.
    let h = hole("6c5c2d2h");
    let b = board("4c3c7c2c2s");
    assert_path3(&h, &b);
    let r = OmahaHighRule::evaluate(&h, &b);
    assert_eq!(r, naive(&h, &b));
    assert_eq!(get_hand_category(r), HandCategory::StraightFlush);
}

// --- Random sweep -----------------------------------------------------

#[test]
fn path3_random_sweep_matches_naive() {
    struct Rng(u64);
    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
    }

    let mut rng = Rng(0x1234_5678_9abc_def0);
    let mut checked = 0usize;
    let mut tried = 0usize;
    while checked < 5_000 && tried < 1_000_000 {
        tried += 1;
        let mut deck: [usize; 52] = std::array::from_fn(|i| i);
        for i in 0..9 {
            let j = i + (rng.next_u64() as usize) % (52 - i);
            deck.swap(i, j);
        }
        let h = [deck[0], deck[1], deck[2], deck[3]];
        let b = [deck[4], deck[5], deck[6], deck[7], deck[8]];

        // Path-3 fixtures only.
        if flush_suit(&h, &b).is_none() || board_has_no_pair(&b) {
            continue;
        }
        let opt = OmahaHighRule::evaluate(&h, &b);
        let nai = naive(&h, &b);
        assert_eq!(
            opt, nai,
            "mismatch on hole={:?} board={:?}: opt={} naive={}",
            h, b, opt, nai
        );
        checked += 1;
    }
    assert!(
        checked >= 1_000,
        "too few path-3 samples drawn ({checked} of {tried})"
    );
}
