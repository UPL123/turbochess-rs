use core::iter::Rev;
use std::{fmt, ops::Neg};

/// Represents a move
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Move(u16);

impl Move {
    // Move flags
    pub const QUIET: usize = 0;
    pub const CAPTURE: usize = 1;
    pub const DOUBLE_PUSH: usize = 2;
    pub const EN_PASSANT: usize = 3;
    pub const CASTLE_00: usize = 4;
    pub const CASTLE_000: usize = 5;
    pub const PR_N: usize = 6;
    pub const PR_B: usize = 7;
    pub const PR_R: usize = 8;
    pub const PR_Q: usize = 9;
    pub const PC_N: usize = 10;
    pub const PC_B: usize = 11;
    pub const PC_R: usize = 12;
    pub const PC_Q: usize = 13;
    pub const EMPTY: Self = Self(0);

    const FROM_MASK: u16 = 0b0000000000111111;
    const TO_MASK: u16 = 0b0000111111000000;
    const FLAG_MASK: u16 = 0b1111000000000000;

    /// Creates a new move
    pub fn new(from: usize, to: usize, flag: usize) -> Self {
        Self((from | to << 6 | flag << 12) as u16)
    }

    /// Gets the start square of the move
    pub fn from(&self) -> usize {
        (self.0 & Self::FROM_MASK) as usize
    }

    /// Gets the destination square of the move
    pub fn to(&self) -> usize {
        ((self.0 & Self::TO_MASK) >> 6) as usize
    }

    /// Gets the flag of the move
    pub fn flag(&self) -> usize {
        ((self.0 & Self::FLAG_MASK) >> 12) as usize
    }

    /// Checks that the move is a capture
    pub fn is_capture(&self) -> bool {
        let flag = self.flag();
        flag == Move::CAPTURE
            || flag == Move::EN_PASSANT
            || flag == Move::PC_B
            || flag == Move::PC_N
            || flag == Move::PC_R
            || flag == Move::PC_Q
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            Square::to_string(self.from()),
            Square::to_string(self.to())
        )
        .unwrap();
        if self.flag() == Move::PR_N || self.flag() == Move::PC_N {
            write!(f, "n").unwrap();
        }
        if self.flag() == Move::PR_B || self.flag() == Move::PC_B {
            write!(f, "b").unwrap();
        }
        if self.flag() == Move::PR_R || self.flag() == Move::PC_R {
            write!(f, "r").unwrap();
        }
        if self.flag() == Move::PR_Q || self.flag() == Move::PC_Q {
            write!(f, "q").unwrap();
        }
        Ok(())
    }
}

/// Represents a list of moves
#[derive(Debug, Copy, Clone)]
pub struct MoveList {
    array: [Move; 218],
    len: usize,
}

impl MoveList {
    /// Creates a new MoveList
    pub fn new() -> Self {
        Self {
            array: [Move::EMPTY; 218],
            len: 0,
        }
    }
    /// Adds a new move by putting its parameters
    pub fn add(&mut self, from: usize, to: usize, flag: usize) {
        self.array[self.len] = Move::new(from, to, flag);
        self.len += 1
    }
    /// Adds a new move
    pub fn add_raw(&mut self, mv: Move) {
        self.array[self.len] = mv;
        self.len += 1
    }
    /// Extends the movelist with a bitboard as destination from a square
    pub fn extend(&mut self, from: usize, mut to: u64, flag: usize) {
        while to != 0 {
            let s = to.get_lsb().bit_scan();
            self.add(from, s, flag);
            to = to.pop_lsb();
        }
    }
    /// Adds promotions
    pub fn add_promotions(&mut self, from: usize, to: usize, capture: bool) {
        if capture {
            self.add(from, to, Move::PC_N);
            self.add(from, to, Move::PC_B);
            self.add(from, to, Move::PC_R);
            self.add(from, to, Move::PC_Q);
        } else {
            self.add(from, to, Move::PR_N);
            self.add(from, to, Move::PR_B);
            self.add(from, to, Move::PR_R);
            self.add(from, to, Move::PR_Q);
        }
    }
    /// Extends to promotions
    pub fn extend_promotions(&mut self, from: usize, mut to: u64, capture: bool) {
        while to != 0 {
            let s = to.get_lsb().bit_scan();
            self.add_promotions(from, s, capture);
            to = to.pop_lsb();
        }
    }
    /// Gets a move
    pub fn get(self, i: usize) -> Move {
        if i <= self.len {
            panic!("Out of bounds");
        }
        self.array[i]
    }
    /// Gets the ammount of moves
    pub fn count(self) -> usize {
        self.len
    }
    /// Counts all the promotions
    pub fn count_promotions(&self) -> usize {
        let mut count = 0;
        for m in &self.array {
            if m == &Move::EMPTY {
                break;
            }
            if m.flag() >= Move::PR_N && m.flag() <= Move::PC_Q {
                count += 1;
            }
        }
        count
    }
    /// Counts all the captures
    pub fn count_captures(&self) -> usize {
        let mut count = 0;
        for m in &self.array {
            if m == &Move::EMPTY {
                break;
            }
            if m.flag() == Move::CAPTURE || (m.flag() >= Move::PC_N && m.flag() <= Move::PC_Q) {
                count += 1;
            }
        }
        count
    }
    /// Counts all the en passants
    pub fn count_enpassants(&self) -> usize {
        let mut count = 0;
        for m in &self.array {
            if m == &Move::EMPTY {
                break;
            }
            if m.flag() == Move::EN_PASSANT {
                count += 1;
            }
        }
        count
    }
    /// Counts all the castles
    pub fn count_castles(&self) -> usize {
        let mut count = 0;
        for m in &self.array {
            if m == &Move::EMPTY {
                break;
            }
            if m.flag() == Move::CASTLE_00 || m.flag() == Move::CASTLE_000 {
                count += 1;
            }
        }
        count
    }
    /// Iterates over the move list in reverse
    pub fn rev(self) -> Rev<MoveListIterator> {
        MoveListIterator {
            list: self,
            index: self.len - 1,
            finish: false,
        }
        .rev()
    }
    /// Filters all moves that are from a square
    pub fn filter_from(&mut self, from: usize) {
        let mut i = 0;
        let mut j = 0;
        while i < self.len {
            if self.array[i].from() == from {
                self.array.swap(i, j);
                i += 1;
                j += 1;
            } else {
                self.array[i] = Move::EMPTY;
                i += 1;
            }
        }
        self.len = j;
    }
    /// Filters all moves that are to a square
    pub fn filter_to(&mut self, to: usize) {
        let mut i = 0;
        let mut j = 0;
        while i < self.len {
            if self.array[i].to() == to {
                self.array.swap(i, j);
                i += 1;
                j += 1;
            } else {
                self.array[i] = Move::EMPTY;
                i += 1;
            }
        }
        self.len = j;
    }
    /// Filters all moves that are from a bitboard
    pub fn filter_from_bb(&mut self, from: u64) {
        let mut i = 0;
        let mut j = 0;
        while i < self.len {
            if (1u64 << self.array[i].from()) & from != 0 {
                self.array.swap(i, j);
                i += 1;
                j += 1;
            } else {
                self.array[i] = Move::EMPTY;
                i += 1;
            }
        }
        self.len = j;
    }
    /// Filters all moves that are to a bitboard
    pub fn filter_to_bb(&mut self, to: u64) {
        let mut i = 0;
        let mut j = 0;
        while i < self.len {
            if (1u64 << self.array[i].to()) & to != 0 {
                self.array.swap(i, j);
                i += 1;
                j += 1;
            } else {
                self.array[i] = Move::EMPTY;
                i += 1;
            }
        }
        self.len = j;
    }
}

impl IntoIterator for MoveList {
    type Item = Move;
    type IntoIter = MoveListIterator;

    fn into_iter(self) -> Self::IntoIter {
        MoveListIterator {
            list: self,
            index: 0,
            finish: false,
        }
    }
}

pub struct MoveListIterator {
    list: MoveList,
    index: usize,
    finish: bool,
}

impl Iterator for MoveListIterator {
    type Item = Move;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.list.len {
            return None;
        }
        let mv = self.list.array[self.index];
        self.index += 1;
        return Some(mv);
    }
}

impl DoubleEndedIterator for MoveListIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.finish {
            return None;
        }
        let mv = self.list.array[self.index];
        if self.index == 0 {
            self.finish = true;
        } else {
            self.index -= 1;
        }
        return Some(mv);
    }
}

pub struct Color;

impl Color {
    pub const WHITE: usize = 0;
    pub const BLACK: usize = 1;
}

pub struct Piece;

impl Piece {
    pub const PAWN: usize = 0;
    pub const KNIGHT: usize = 1;
    pub const BISHOP: usize = 2;
    pub const ROOK: usize = 3;
    pub const QUEEN: usize = 4;
    pub const KING: usize = 5;
}

impl Piece {
    pub fn from_char(ch: char) -> usize {
        match ch.to_ascii_lowercase() {
            'p' => 0,
            'n' => 1,
            'b' => 2,
            'r' => 3,
            'q' => 4,
            'k' => 5,
            _ => unreachable!("Invalid piece character"),
        }
    }
    pub fn to_char(p: usize) -> char {
        match p {
            0 => 'p',
            1 => 'n',
            2 => 'b',
            3 => 'r',
            4 => 'q',
            5 => 'k',
            _ => unreachable!("Invalid piece type"),
        }
    }
}

/// Represents a direction
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum Direction {
    North = 8,
    NorthEast = 9,
    East = 1,
    SouthEast = -7,
    South = -8,
    SouthWest = -9,
    West = -1,
    NorthWest = 7,
    North2 = 16,
    South2 = -16,
}
impl Neg for Direction {
    type Output = Self;
    fn neg(self) -> Self::Output {
        return if self == Self::North {
            Self::South
        } else if self == Self::South {
            Self::North
        } else if self == Self::North2 {
            Self::South2
        } else if self == Self::South2 {
            Self::North2
        } else if self == Self::East {
            Self::West
        } else if self == Self::West {
            Self::East
        } else if self == Self::NorthEast {
            Self::SouthWest
        } else if self == Self::NorthWest {
            Self::SouthEast
        } else if self == Self::SouthWest {
            Self::NorthEast
        } else if self == Self::SouthEast {
            Self::NorthWest
        } else {
            unreachable!()
        };
    }
}
impl Direction {
    /// Gets the relative direction in the look of a specific color
    pub fn relative(self, color: usize) -> Self {
        return if color == Color::BLACK { -self } else { self };
    }
}

/// Represents helpers for bitboards
pub struct BitBoard;

impl BitBoard {
    pub const FILE_A: u64 = 0x101010101010101;
    pub const FILE_B: u64 = 0x202020202020202;
    pub const FILE_C: u64 = 0x404040404040404;
    pub const FILE_D: u64 = 0x808080808080808;
    pub const FILE_E: u64 = 0x1010101010101010;
    pub const FILE_F: u64 = 0x2020202020202020;
    pub const FILE_G: u64 = 0x4040404040404040;
    pub const FILE_H: u64 = 0x8080808080808080;

    pub const RANK_1: u64 = 0xff;
    pub const RANK_2: u64 = 0xff00;
    pub const RANK_3: u64 = 0xff0000;
    pub const RANK_4: u64 = 0xff000000;
    pub const RANK_5: u64 = 0xff00000000;
    pub const RANK_6: u64 = 0xff0000000000;
    pub const RANK_7: u64 = 0xff000000000000;
    pub const RANK_8: u64 = 0xff00000000000000;

    /// Prints a bitboard
    pub fn print(mut bb: u64) {
        println!("  +-----------------+");
        let mut rank = [0; 8];
        let mut i = 0;
        while bb > 0 {
            rank[i] = bb & 0xff;
            bb >>= 8;
            i += 1;
        }
        for i in (0..8).rev() {
            print!("{} | ", i + 1);
            for j in 0..8 {
                print!("{} ", (rank[i] >> j) & 1);
            }
            println!("|");
        }
        println!("  +-----------------+\n    a b c d e f g h");
    }

    /// Shifts a bitboard into a direction
    pub fn shift_dir(bb: u64, dir: Direction) -> u64 {
        return if dir == Direction::North {
            bb << 8
        } else if dir == Direction::South {
            bb >> 8
        } else if dir == Direction::North2 {
            bb << 16
        } else if dir == Direction::South2 {
            bb >> 16
        } else if dir == Direction::East {
            (bb & !Self::FILE_H) << 1
        } else if dir == Direction::West {
            (bb & !Self::FILE_A) >> 1
        } else if dir == Direction::NorthEast {
            (bb & !Self::FILE_H) << 9
        } else if dir == Direction::NorthWest {
            (bb & !Self::FILE_A) << 7
        } else if dir == Direction::SouthEast {
            (bb & !Self::FILE_H) >> 7
        } else if dir == Direction::SouthWest {
            (bb & !Self::FILE_A) >> 9
        } else {
            unreachable!()
        };
    }

    /// Gets the relative rank in the look of a specific color
    pub fn relative_rank(rank: usize, color: usize) -> u64 {
        let num = if color == Color::WHITE {
            rank - 1
        } else {
            8 - rank
        };
        Self::RANK_1 << (num * 8)
    }
}

pub struct BitBoardSubsetIter {
    set: u64,
    subset: u64,
    finished: bool,
}

impl Iterator for BitBoardSubsetIter {
    type Item = u64;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let current = self.subset;
        self.subset = self.subset.wrapping_sub(self.set) & self.set;
        self.finished = self.subset == 0;
        Some(current)
    }
}

pub struct Square;

impl Square {
    // Square definitions
    pub const A1: usize = 0;
    pub const B1: usize = 1;
    pub const C1: usize = 2;
    pub const D1: usize = 3;
    pub const E1: usize = 4;
    pub const F1: usize = 5;
    pub const G1: usize = 6;
    pub const H1: usize = 7;
    pub const A2: usize = 8;
    pub const B2: usize = 9;
    pub const C2: usize = 10;
    pub const D2: usize = 11;
    pub const E2: usize = 12;
    pub const F2: usize = 13;
    pub const G2: usize = 14;
    pub const H2: usize = 15;
    pub const A3: usize = 16;
    pub const B3: usize = 17;
    pub const C3: usize = 18;
    pub const D3: usize = 19;
    pub const E3: usize = 20;
    pub const F3: usize = 21;
    pub const G3: usize = 22;
    pub const H3: usize = 23;
    pub const A4: usize = 24;
    pub const B4: usize = 25;
    pub const C4: usize = 26;
    pub const D4: usize = 27;
    pub const E4: usize = 28;
    pub const F4: usize = 29;
    pub const G4: usize = 30;
    pub const H4: usize = 31;
    pub const A5: usize = 32;
    pub const B5: usize = 33;
    pub const C5: usize = 34;
    pub const D5: usize = 35;
    pub const E5: usize = 36;
    pub const F5: usize = 37;
    pub const G5: usize = 38;
    pub const H5: usize = 39;
    pub const A6: usize = 40;
    pub const B6: usize = 41;
    pub const C6: usize = 42;
    pub const D6: usize = 43;
    pub const E6: usize = 44;
    pub const F6: usize = 45;
    pub const G6: usize = 46;
    pub const H6: usize = 47;
    pub const A7: usize = 48;
    pub const B7: usize = 49;
    pub const C7: usize = 50;
    pub const D7: usize = 51;
    pub const E7: usize = 52;
    pub const F7: usize = 53;
    pub const G7: usize = 54;
    pub const H7: usize = 55;
    pub const A8: usize = 56;
    pub const B8: usize = 57;
    pub const C8: usize = 58;
    pub const D8: usize = 59;
    pub const E8: usize = 60;
    pub const F8: usize = 61;
    pub const G8: usize = 62;
    pub const H8: usize = 63;

    /// Gets a square from a string
    pub fn from_str(sq: &str) -> usize {
        let file = sq.chars().nth(0).unwrap().to_ascii_lowercase() as usize - 'a' as usize;
        let rank = sq.chars().nth(1).unwrap().to_digit(10).unwrap() as usize - 1;
        rank * 8 + file
    }

    /// Converts the square into a string
    pub fn to_string(sq: usize) -> String {
        let file = (sq % 8) as u8 + b'a';
        let rank = (sq / 8) as u8 + b'1';
        format!("{}{}", file as char, rank as char)
    }
}

pub trait BitHelpers {
    type Item;

    fn get_lsb(&self) -> Self::Item;
    fn pop_lsb(&self) -> Self::Item;
    fn bit_count(&self) -> usize;
    fn bit_scan(&self) -> usize;
    fn offset(&self, offset: usize, color: usize) -> Self::Item;
}

macro_rules! bit_helpers_implementation {
    ($type:ident) => {
        impl BitHelpers for $type {
            type Item = $type;

            /// Extracts the lowest set isolated bit.
            ///
            /// More about asm instruction: <https://www.felixcloutier.com/x86/blsi>
            #[inline(always)]
            fn get_lsb(&self) -> Self::Item {
                self & self.wrapping_neg()
            }

            /// Resets the lowest set bit.
            ///
            /// More about asm instruction: <https://www.felixcloutier.com/x86/blsr>
            #[inline(always)]
            fn pop_lsb(&self) -> Self::Item {
                self & (self - 1)
            }

            /// Counts the number of set bits.
            ///
            /// More about asm instruction: <https://www.felixcloutier.com/x86/popcnt>
            #[inline(always)]
            fn bit_count(&self) -> usize {
                self.count_ones() as usize
            }

            /// Gets an index of the first set bit by counting trailing zero bits.
            ///
            /// More about asm instruction: <https://www.felixcloutier.com/x86/tzcnt>
            #[inline(always)]
            fn bit_scan(&self) -> usize {
                self.trailing_zeros() as usize
            }

            /// Offsets a number to the left as white or to the right as black
            fn offset(&self, offset: usize, color: usize) -> Self::Item {
                return if color == 0 {
                    self << offset
                } else {
                    self >> offset
                };
            }
        }
    };
}

bit_helpers_implementation!(u64);
