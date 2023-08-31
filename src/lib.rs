//! TurboChess library
//!
//! TurboChess is a move generator for chess. It supports:
//!
//!
//! * PEXT bitboards (emulated)
//! * Make and Undo Position
//! * Zobrist hashing
//! * FEN support
//!
//! To get started, create a new [Position] and now you can work with legal moves
//!
//! ```rs
//! let pos: Position = Position::default(); // Loads the initial position
//! // Or you can also use another position
//! // let pos: Position = Position::from_str("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
//!
//! let legals: Vec<Move> = pos.legal(); // Gets all legal moves in the position
//! for mv in legals {
//!     println!("{mv}"); // Prints the move in the UCI format
//! }
//!
//! println!("Legal count: {}", legals.count());
//! ```
//!

mod lookup;
mod testing;
pub mod types;

use std::{fmt, str::FromStr};

use lookup::{
    between, d12_moves, hv_moves, line, oo_blockers, ooo_blockers, ooo_danger, D12_MASKS,
    D12_MASKS_2, HV_MASKS, HV_MASKS_2, KING_MASK, KNIGHT_MASK, PAWN_ATTACKS, ZOBRIST_CASTLE,
    ZOBRIST_EP, ZOBRIST_PIECES,
};
use types::{BitBoard, Color, Move, MoveList, Piece, Square};

use crate::types::{BitHelpers, Direction};

/// Represents the state of the game
#[derive(Debug, Clone, Copy)]
pub struct State {
    pub turn: usize,
    pub castling: u8,
    pub captured: Option<usize>,
    pub ep: Option<usize>,
    pub hm: usize,
    pub fm: usize,
}

impl State {
    pub const WHITE_00: u8 = 0b0001;
    pub const WHITE_000: u8 = 0b0010;
    pub const WHITE_CASTLING: u8 = 0b0011;
    pub const BLACK_00: u8 = 0b0100;
    pub const BLACK_000: u8 = 0b1000;
    pub const BLACK_CASTLING: u8 = 0b1100;
    pub const ALL_CASTLING: u8 = 0b1111;
    pub const CASTLINGS: [u8; 2] = [Self::WHITE_CASTLING, Self::BLACK_CASTLING];
    pub const SHORT: [u8; 2] = [Self::WHITE_00, Self::BLACK_00];
    pub const SHORT_TARGET: [usize; 2] = [Square::G1, Square::G8];
    pub const KING_START: [usize; 2] = [Square::E1, Square::E8];
    pub const SHORT_ROOK: [usize; 2] = [Square::H1, Square::H8];
    pub const SHORT_KING_TARGET: [usize; 2] = [Square::G1, Square::G8];
    pub const LONG: [u8; 2] = [Self::WHITE_000, Self::BLACK_000];
    pub const LONG_KING_TARGET: [usize; 2] = [Square::C1, Square::C8];
    pub const LONG_ROOK: [usize; 2] = [Square::A1, Square::A8];
    /// Creates an empty state
    pub fn new() -> Self {
        Self {
            turn: 0,
            castling: 0,
            captured: None,
            ep: None,
            hm: 0,
            fm: 0,
        }
    }
    /// Checks if you can castle one side
    pub fn can_castle(self, side: u8) -> bool {
        self.castling & side != 0
    }
}

/// Represents a position
#[derive(Debug, Clone, Copy)]
pub struct Position {
    ply: usize,
    pieces_bb: [[u64; 6]; 2],
    history: [State; 216],
    hash: u64,
    pin_hv: u64,
    pin_d12: u64,
    danger: u64,
    checkmask: u64,
}

impl Position {
    /// Creates a new position
    pub fn new() -> Self {
        Self {
            ply: 0,
            pieces_bb: [[0; 6]; 2],
            history: [State::new(); 216],
            hash: 0,
            pin_hv: 0,
            pin_d12: 0,
            danger: 0,
            checkmask: 0,
        }
    }
    /// Moves a piece from a square to another.
    pub fn move_quiet(&mut self, from: usize, to: usize) {
        let c = self.color_on(from).unwrap();
        let p = self.piece_on(from).unwrap();
        self.pieces_bb[c][p] ^= (1u64 << from) | (1u64 << to);
        self.hash ^= ZOBRIST_PIECES[c][p][from];
        self.hash ^= ZOBRIST_PIECES[c][p][to];
    }
    /// Sets a square. NOTE: It replaces the piece on the square
    pub fn set_square(&mut self, square: usize, piece: usize, color: usize) {
        self.unset_square(square);
        self.pieces_bb[color][piece] |= 1u64 << square;
        self.hash ^= ZOBRIST_PIECES[color][piece][square];
    }
    /// Updates the checkmask and pinned mask
    pub fn update_checks(&mut self) {
        let (c, hv, d12) = self.check_and_pin();
        self.checkmask = c;
        self.pin_hv = hv;
        self.pin_d12 = d12;
        self.danger = self.attacks();
    }
    /// Unsets a square
    pub fn unset_square(&mut self, square: usize) {
        for c in [0, 1] {
            for p in 0..6 {
                self.pieces_bb[c][p] &= !(1u64 << square);
                self.hash ^= ZOBRIST_PIECES[c][p][square];
            }
        }
    }
    /// Gets the color of a piece in a specific square
    pub fn color_on(self, square: usize) -> Option<usize> {
        for c in [0, 1] {
            for p in 0..6 {
                if self.pieces_bb[c][p] & (1u64 << square) != 0 {
                    return Some(c);
                }
            }
        }
        None
    }
    /// Gets the type of a piece in a specific square
    pub fn piece_on(self, square: usize) -> Option<usize> {
        for c in [0, 1] {
            for p in 0..6 {
                if self.pieces_bb[c][p] & (1u64 << square) != 0 {
                    return Some(p);
                }
            }
        }
        None
    }
    /// Gets the zobrist hashing of the actual position
    pub fn hash(self, enpassant: bool) -> u64 {
        let piece_hash = self.hash;
        let state = self.actual_state();
        let ep_hash = if let Some(ep) = state.ep {
            if enpassant {
                ZOBRIST_EP[state.turn][ep]
            } else {
                0
            }
        } else {
            0
        };
        let mut castle_hash = 0;
        if state.can_castle(State::WHITE_00) {
            castle_hash ^= ZOBRIST_CASTLE[state.turn][0];
        }
        if state.can_castle(State::WHITE_000) {
            castle_hash ^= ZOBRIST_CASTLE[state.turn][1];
        }
        if state.can_castle(State::BLACK_00) {
            castle_hash ^= ZOBRIST_CASTLE[state.turn][2];
        }
        if state.can_castle(State::BLACK_000) {
            castle_hash ^= ZOBRIST_CASTLE[state.turn][3];
        }
        piece_hash ^ ep_hash ^ castle_hash
    }
    /// Gets the actual state of the game
    pub fn actual_state(&self) -> State {
        self.history[self.ply]
    }
    /// Gets a bitboard of all the pieces of a specific color and type

    #[inline(always)]
    pub fn bb_of(&self, color: usize, piece: usize) -> u64 {
        self.pieces_bb[color][piece]
    }
    /// Gets a bitboard of all the pieces of a specific type

    #[inline(always)]
    pub fn pieces(&self, piece: usize) -> u64 {
        let mut bb = 0u64;
        for c in [0, 1] {
            bb |= self.pieces_bb[c][piece];
        }
        bb
    }
    /// Gets a bitboard of all the pieces of a specific color

    #[inline(always)]
    pub fn colors(&self, color: usize) -> u64 {
        let mut bb = 0u64;
        for p in 0..6 {
            bb |= self.pieces_bb[color][p];
        }
        bb
    }
    /// Returns the square of the king of a certain color

    #[inline(always)]
    pub fn king(&self, color: usize) -> usize {
        self.pieces_bb[color][Piece::KING].bit_scan()
    }
    /// All sliding pieces that can move horizontally and vertically

    #[inline(always)]
    pub fn hv_sliders(&self, color: usize) -> u64 {
        self.bb_of(color, Piece::ROOK) | self.bb_of(color, Piece::QUEEN)
    }
    /// All sliding pieces that can move diagonally

    #[inline(always)]
    pub fn d12_sliders(&self, color: usize) -> u64 {
        self.bb_of(color, Piece::BISHOP) | self.bb_of(color, Piece::QUEEN)
    }
    /// Makes a move without checking its legability

    #[inline(always)]
    pub fn make_move(&mut self, mv: Move) {
        let state = self.actual_state();
        self.ply += 1;
        self.history[self.ply].turn = 1 - state.turn;
        self.history[self.ply].fm = state.fm;
        self.history[self.ply].hm = state.hm;
        self.history[self.ply].castling = state.castling;
        if state.turn == Color::BLACK {
            self.history[self.ply].fm += 1
        }
        let mut hm_reset = false;
        if self.piece_on(mv.from()).unwrap() == Piece::PAWN {
            hm_reset = true;
        }
        match mv.flag() {
            Move::QUIET => {
                // If we move the king, remove all castlings
                if self.piece_on(mv.from()).unwrap() == Piece::KING {
                    self.history[self.ply].castling &= !State::CASTLINGS[state.turn]
                }
                if self.piece_on(mv.from()).unwrap() == Piece::ROOK {
                    if state.can_castle(State::SHORT[state.turn])
                        && mv.from() == State::SHORT_ROOK[state.turn]
                    {
                        self.history[self.ply].castling &= !State::SHORT[state.turn];
                    }
                    if state.can_castle(State::LONG[state.turn])
                        && mv.from() == State::LONG_ROOK[state.turn]
                    {
                        self.history[self.ply].castling &= !State::LONG[state.turn];
                    }
                }
                self.move_quiet(mv.from(), mv.to())
            }
            Move::DOUBLE_PUSH => {
                self.move_quiet(mv.from(), mv.to());
                self.history[self.ply].ep = Some(
                    (mv.from() as i32 + Direction::relative(Direction::North, state.turn) as i32)
                        as usize,
                );
            }
            Move::CASTLE_00 => {
                if state.turn == Color::WHITE {
                    self.move_quiet(Square::E1, Square::G1);
                    self.move_quiet(Square::H1, Square::F1);
                    // Remove the castling
                    self.history[self.ply].castling &= !State::WHITE_CASTLING;
                } else {
                    self.move_quiet(Square::E8, Square::G8);
                    self.move_quiet(Square::H8, Square::F8);
                    // Remove the castling
                    self.history[self.ply].castling &= !State::BLACK_CASTLING;
                }
            }
            Move::CASTLE_000 => {
                if state.turn == Color::WHITE {
                    self.move_quiet(Square::E1, Square::C1);
                    self.move_quiet(Square::A1, Square::D1);
                    // Remove the castling
                    self.history[self.ply].castling &= !State::WHITE_CASTLING;
                } else {
                    self.move_quiet(Square::E8, Square::C8);
                    self.move_quiet(Square::A8, Square::D8);
                    // Remove the castling
                    self.history[self.ply].castling &= !State::BLACK_CASTLING;
                }
            }
            Move::EN_PASSANT => {
                self.move_quiet(mv.from(), mv.to());
                self.unset_square(
                    (mv.to() as i32 + Direction::relative(Direction::South, state.turn) as i32)
                        as usize,
                )
            }
            Move::PR_N => {
                self.unset_square(mv.from());
                self.set_square(mv.to(), Piece::KNIGHT, state.turn);
            }
            Move::PR_B => {
                self.unset_square(mv.from());
                self.set_square(mv.to(), Piece::BISHOP, state.turn);
            }
            Move::PR_R => {
                self.unset_square(mv.from());
                self.set_square(mv.to(), Piece::ROOK, state.turn);
            }
            Move::PR_Q => {
                self.unset_square(mv.from());
                self.set_square(mv.to(), Piece::QUEEN, state.turn);
            }
            Move::PC_N => {
                hm_reset = true;
                self.unset_square(mv.from());
                self.history[self.ply].captured = Some(self.piece_on(mv.to()).unwrap());
                self.set_square(mv.to(), Piece::KNIGHT, state.turn);
                // If captures a rook that can caslte, then remove that castle
                if state.can_castle(State::WHITE_00) {
                    if mv.to() == Square::H1 {
                        self.history[self.ply].castling &= !State::WHITE_00
                    }
                }
                if state.can_castle(State::WHITE_000) {
                    if mv.to() == Square::A1 {
                        self.history[self.ply].castling &= !State::WHITE_000
                    }
                }
                if state.can_castle(State::BLACK_00) {
                    if mv.to() == Square::H8 {
                        self.history[self.ply].castling &= !State::BLACK_00
                    }
                }
                if state.can_castle(State::BLACK_000) {
                    if mv.to() == Square::A8 {
                        self.history[self.ply].castling &= !State::BLACK_000
                    }
                }
            }
            Move::PC_B => {
                hm_reset = true;
                self.unset_square(mv.from());
                self.history[self.ply].captured = Some(self.piece_on(mv.to()).unwrap());
                self.unset_square(mv.to());
                self.set_square(mv.to(), Piece::BISHOP, state.turn);
                // If captures a rook that can caslte, then remove that castle
                if state.can_castle(State::WHITE_00) {
                    if mv.to() == Square::H1 {
                        self.history[self.ply].castling &= !State::WHITE_00
                    }
                }
                if state.can_castle(State::WHITE_000) {
                    if mv.to() == Square::A1 {
                        self.history[self.ply].castling &= !State::WHITE_000
                    }
                }
                if state.can_castle(State::BLACK_00) {
                    if mv.to() == Square::H8 {
                        self.history[self.ply].castling &= !State::BLACK_00
                    }
                }
                if state.can_castle(State::BLACK_000) {
                    if mv.to() == Square::A8 {
                        self.history[self.ply].castling &= !State::BLACK_000
                    }
                }
            }
            Move::PC_R => {
                hm_reset = true;
                self.unset_square(mv.from());
                self.history[self.ply].captured = Some(self.piece_on(mv.to()).unwrap());
                self.unset_square(mv.to());
                self.set_square(mv.to(), Piece::ROOK, state.turn);
                // If captures a rook that can caslte, then remove that castle
                if state.can_castle(State::WHITE_00) {
                    if mv.to() == Square::H1 {
                        self.history[self.ply].castling &= !State::WHITE_00
                    }
                }
                if state.can_castle(State::WHITE_000) {
                    if mv.to() == Square::A1 {
                        self.history[self.ply].castling &= !State::WHITE_000
                    }
                }
                if state.can_castle(State::BLACK_00) {
                    if mv.to() == Square::H8 {
                        self.history[self.ply].castling &= !State::BLACK_00
                    }
                }
                if state.can_castle(State::BLACK_000) {
                    if mv.to() == Square::A8 {
                        self.history[self.ply].castling &= !State::BLACK_000
                    }
                }
            }
            Move::PC_Q => {
                hm_reset = true;
                self.unset_square(mv.from());
                self.history[self.ply].captured = Some(self.piece_on(mv.to()).unwrap());
                self.unset_square(mv.to());
                self.set_square(mv.to(), Piece::QUEEN, state.turn);
                // If captures a rook that can caslte, then remove that castle
                if state.can_castle(State::WHITE_00) {
                    if mv.to() == Square::H1 {
                        self.history[self.ply].castling &= !State::WHITE_00
                    }
                }
                if state.can_castle(State::WHITE_000) {
                    if mv.to() == Square::A1 {
                        self.history[self.ply].castling &= !State::WHITE_000
                    }
                }
                if state.can_castle(State::BLACK_00) {
                    if mv.to() == Square::H8 {
                        self.history[self.ply].castling &= !State::BLACK_00
                    }
                }
                if state.can_castle(State::BLACK_000) {
                    if mv.to() == Square::A8 {
                        self.history[self.ply].castling &= !State::BLACK_000
                    }
                }
            }
            Move::CAPTURE => {
                hm_reset = true;
                // If we move the king, remove all castlings
                if self.piece_on(mv.from()).unwrap() == Piece::KING {
                    self.history[self.ply].castling &= !State::CASTLINGS[state.turn]
                } else {
                    // If captures a rook that can caslte, then remove that castle
                    if state.can_castle(State::WHITE_00) {
                        if mv.to() == Square::H1 {
                            self.history[self.ply].castling &= !State::WHITE_00
                        }
                    }
                    if state.can_castle(State::WHITE_000) {
                        if mv.to() == Square::A1 {
                            self.history[self.ply].castling &= !State::WHITE_000
                        }
                    }
                    if state.can_castle(State::BLACK_00) {
                        if mv.to() == Square::H8 {
                            self.history[self.ply].castling &= !State::BLACK_00
                        }
                    }
                    if state.can_castle(State::BLACK_000) {
                        if mv.to() == Square::A8 {
                            self.history[self.ply].castling &= !State::BLACK_000
                        }
                    }
                }
                if self.piece_on(mv.from()).unwrap() == Piece::ROOK {
                    if state.can_castle(State::SHORT[state.turn])
                        && mv.from() == State::SHORT_ROOK[state.turn]
                    {
                        self.history[self.ply].castling &= !State::SHORT[state.turn];
                    }
                    if state.can_castle(State::LONG[state.turn])
                        && mv.from() == State::LONG_ROOK[state.turn]
                    {
                        self.history[self.ply].castling &= !State::LONG[state.turn];
                    }
                }
                self.history[self.ply].captured = Some(self.piece_on(mv.to()).unwrap());
                self.unset_square(mv.to());
                self.move_quiet(mv.from(), mv.to());
            }
            _ => {
                unreachable!("Invalid move flag")
            }
        }
        if hm_reset {
            self.history[self.ply].hm = 0
        } else {
            self.history[self.ply].hm += 1
        }
        self.update_checks();
    }
    /// Undoes a move

    #[inline(always)]
    pub fn undo_move(&mut self, mv: Move) {
        // Replace the actual state
        let state = self.actual_state();
        self.history[self.ply] = State::new();
        self.ply -= 1;
        match mv.flag() {
            Move::QUIET | Move::DOUBLE_PUSH => self.move_quiet(mv.to(), mv.from()),
            Move::CASTLE_00 => {
                if 1 - state.turn == Color::WHITE {
                    self.move_quiet(Square::G1, Square::E1);
                    self.move_quiet(Square::F1, Square::H1);
                } else {
                    self.move_quiet(Square::G8, Square::E8);
                    self.move_quiet(Square::F8, Square::H8);
                }
            }
            Move::CASTLE_000 => {
                if 1 - state.turn == Color::WHITE {
                    self.move_quiet(Square::C1, Square::E1);
                    self.move_quiet(Square::D1, Square::A1);
                } else {
                    self.move_quiet(Square::C8, Square::E8);
                    self.move_quiet(Square::D8, Square::A8);
                }
            }
            Move::EN_PASSANT => {
                self.move_quiet(mv.to(), mv.from());
                self.set_square(
                    (mv.to() as i32 + Direction::relative(Direction::South, 1 - state.turn) as i32)
                        as usize,
                    Piece::PAWN,
                    state.turn,
                )
            }
            Move::PR_N | Move::PR_B | Move::PR_R | Move::PR_Q => {
                self.unset_square(mv.to());
                self.set_square(mv.from(), Piece::PAWN, 1 - state.turn)
            }
            Move::PC_N | Move::PC_B | Move::PC_R | Move::PC_Q => {
                self.unset_square(mv.to());
                self.set_square(mv.to(), state.captured.unwrap(), state.turn);
                self.set_square(mv.from(), Piece::PAWN, 1 - state.turn);
            }
            Move::CAPTURE => {
                self.move_quiet(mv.to(), mv.from());
                self.set_square(mv.to(), state.captured.unwrap(), state.turn);
            }
            _ => {
                unreachable!("Invalid move flag")
            }
        }

        self.update_checks();
    }
    /// Gets the FEN notation of the current position

    #[inline(always)]
    pub fn fen(&self) -> String {
        let state = self.actual_state();
        let mut pieces = String::new();
        for r in (0..8).rev() {
            let mut em = 0;
            for f in 0..8 {
                let s = r * 8 + f;
                if let Some(p) = self.piece_on(s) {
                    if em != 0 {
                        pieces.push_str(&em.to_string());
                        em = 0;
                    }
                    let c = self.color_on(s).unwrap();
                    let mut chr = Piece::to_char(p);
                    if c == Color::WHITE {
                        chr = chr.to_ascii_uppercase()
                    }
                    pieces.push(chr);
                } else {
                    em += 1
                }
            }
            if em != 0 {
                pieces.push_str(&em.to_string())
            }
            if r != 0 {
                pieces.push_str("/")
            }
        }
        let mut castling = String::from("-");
        if state.castling != 0 {
            castling = String::new();

            if state.can_castle(State::WHITE_00) {
                castling.push('K');
            }
            if state.can_castle(State::WHITE_000) {
                castling.push('Q');
            }
            if state.can_castle(State::BLACK_00) {
                castling.push('k');
            }
            if state.can_castle(State::BLACK_000) {
                castling.push('q');
            }
        }
        let mut ep = String::from("-");
        if let Some(s) = state.ep {
            ep = Square::to_string(s)
        }
        format!(
            "{pieces} {} {castling} {ep} {} {}",
            if state.turn == Color::WHITE { "w" } else { "b" },
            state.hm,
            state.fm
        )
    }
    /// Gets all the opponent attackers from a square
    #[inline(always)]
    pub fn attackers_from(&self, s: usize, color: usize, occ: u64) -> u64 {
        (PAWN_ATTACKS[1 - color][s] & self.pieces_bb[color][Piece::PAWN])
            | (KNIGHT_MASK[s]) & self.pieces_bb[color][Piece::KNIGHT]
            | (d12_moves(s, occ)) & self.d12_sliders(color)
            | (hv_moves(s, occ)) & self.hv_sliders(color)
    }
    /// Gets the occupancy of the board
    #[inline(always)]
    pub fn occupancy(&self) -> u64 {
        self.colors(Color::WHITE) | self.colors(Color::BLACK)
    }
    /// Gets the mask where you can move
    #[inline(always)]
    pub fn checkmask(&self) -> u64 {
        self.checkmask
    }
    /// Gets the mask with all pinned pieces
    #[inline(always)]
    pub fn pinned(&self) -> u64 {
        self.pin_d12 | self.pin_hv
    }
    /// Gets all the attacked squares from the opponent
    #[inline(always)]
    pub fn attacks(&self) -> u64 {
        let mut attacks = 0;
        let state = self.actual_state();
        let o_king = self.king(state.turn);
        let e_king = self.king(1 - state.turn);
        let occ = self.occupancy() & !(1u64 << o_king); // Remove our king for fixing check slider

        // General use variables
        let mut b1 = 0u64;
        let mut s = 0usize;

        // King attacks
        attacks |= KING_MASK[e_king];

        // Pawn attacks
        b1 = self.pieces_bb[1 - state.turn][Piece::PAWN];
        while b1 != 0 {
            s = b1.bit_scan();
            attacks |= PAWN_ATTACKS[1 - state.turn][s];
            b1 = b1.pop_lsb();
        }

        // Knight attacks
        b1 = self.pieces_bb[1 - state.turn][Piece::KNIGHT];
        while b1 != 0 {
            s = b1.bit_scan();
            attacks |= KNIGHT_MASK[s];
            b1 = b1.pop_lsb();
        }

        // HV attacks
        b1 = self.hv_sliders(1 - state.turn);
        while b1 != 0 {
            s = b1.bit_scan();
            attacks |= hv_moves(s, occ);
            b1 = b1.pop_lsb();
        }

        // D12 attacks
        b1 = self.d12_sliders(1 - state.turn);
        while b1 != 0 {
            s = b1.bit_scan();
            attacks |= d12_moves(s, occ);
            b1 = b1.pop_lsb();
        }

        attacks
    }
    /// Calculates the checks and pins at the same time
    fn check_and_pin(&self) -> (u64, u64, u64) {
        let mut checkmask = 0u64;
        let mut check_count = 0;
        let mut pin_hv = 0u64;
        let mut pin_d12 = 0u64;

        // General use variables
        let mut b1 = 0u64;
        let mut b2 = 0u64;
        let mut s = 0usize;
        let state = self.actual_state();
        let king = self.king(state.turn);

        // Get all orthogonal sliders
        let mut e_hv = self.hv_sliders(1 - state.turn);

        // Get all diagonal sliders
        let mut e_d12 = self.d12_sliders(1 - state.turn);

        // Checks from knights of pawns
        checkmask |= (KNIGHT_MASK[king] & self.pieces_bb[1 - state.turn][Piece::KNIGHT])
            | (PAWN_ATTACKS[state.turn][king] & self.pieces_bb[1 - state.turn][Piece::PAWN]);
        check_count = checkmask.bit_count();
        while e_hv != 0 {
            s = e_hv.bit_scan();
            b1 = between(king, s) & HV_MASKS[king]; // Use a mask to filter queen moves that move diagonally
            b2 = line(king, s) & HV_MASKS_2[king]; // Use a mask to filter queen moves that move diagonally

            // Quich check: Skip it if it isn't checking the king
            if b2 == 0 {
                e_hv = e_hv.pop_lsb();
                continue;
            }
            // If no square between
            if b1 == 0 {
                checkmask |= 1u64 << s;
                check_count += 1;
            } else {
                // If there is an enemy piece between, then skip it
                if b1 & self.colors(1 - state.turn) != 0 {
                    e_hv = e_hv.pop_lsb();
                    continue;
                }
                // If none of our pieces is between
                if b1 & self.colors(state.turn) == 0 {
                    checkmask |= 1u64 << s;
                    checkmask |= b1;
                    check_count += 1;
                } else if (b1 & self.colors(state.turn)).bit_count() == 1 {
                    // If 1 piece is between, then it's pinned
                    pin_hv |= 1u64 << s;
                    pin_hv |= b1;
                }
            }

            e_hv = e_hv.pop_lsb();
        }
        while e_d12 != 0 {
            s = e_d12.bit_scan();
            b1 = between(king, s) & D12_MASKS[king]; // Use a mask to filter queen moves that move orthagonally
            b2 = line(king, s) & D12_MASKS_2[king]; // Use a mask to filter queen moves that move orthagonally

            // Quich check: Skip it if it isn't checking the king
            if b2 == 0 {
                e_d12 = e_d12.pop_lsb();
                continue;
            }
            // If no square between
            if b1 == 0 {
                checkmask |= 1u64 << s;
                check_count += 1;
            } else {
                // If there is an enemy piece between, then skip it
                if b1 & self.colors(1 - state.turn) != 0 {
                    e_d12 = e_d12.pop_lsb();
                    continue;
                }
                // If none of our pieces is between
                if b1 & self.colors(state.turn) == 0 {
                    checkmask |= 1u64 << s;
                    checkmask |= b1;
                    check_count += 1;
                } else if (b1 & self.colors(state.turn)).bit_count() == 1 {
                    // If 1 piece is between, then it's pinned
                    pin_d12 |= 1u64 << s;
                    pin_d12 |= b1;
                }
            }
            e_d12 = e_d12.pop_lsb();
        }
        // If no check, fill the checkmask
        if check_count == 0 {
            checkmask = u64::MAX;
        }
        // If is a double check, set it to 0
        if check_count > 1 {
            checkmask = 0;
        }

        (checkmask, pin_hv, pin_d12)
    }
    /// Check if the actual player is on check
    #[inline(always)]
    pub fn in_check(&self) -> bool {
        self.checkmask != u64::MAX
    }
    /// Calculates all the legal moves in the position
    #[inline(always)]
    pub fn legal(&self) -> MoveList {
        let mut list = MoveList::new();
        let state = self.actual_state();
        let o_king = self.king(state.turn);

        // Useful bitboards
        let occ = self.occupancy();
        let en = self.colors(1 - state.turn);
        let em = !occ;

        // General use bitboards
        let mut s = 0usize;
        let mut b1 = 0u64;
        let mut b2 = 0u64;
        let mut b3 = 0u64;

        // Generate king moves first
        b1 = KING_MASK[o_king] & !self.danger;
        list.extend(o_king, b1 & en, Move::CAPTURE);
        list.extend(o_king, b1 & em, Move::QUIET);

        // Quick check: If is a double check, only return king moves
        if self.checkmask == 0 {
            return list;
        }

        let pinned = self.pin_hv | self.pin_d12;

        // Knight moves
        b1 = self.pieces_bb[state.turn][Piece::KNIGHT] & !pinned;
        while b1 != 0 {
            s = b1.bit_scan();
            list.extend(s, KNIGHT_MASK[s] & self.checkmask & en, Move::CAPTURE);
            list.extend(s, KNIGHT_MASK[s] & self.checkmask & em, Move::QUIET);
            b1 = b1.pop_lsb();
        }

        // HV moves that are pinned horizontally
        b1 = self.hv_sliders(state.turn) & !self.pin_d12 & self.pin_hv;
        while b1 != 0 {
            s = b1.bit_scan();
            list.extend(
                s,
                hv_moves(s, occ) & self.checkmask & self.pin_hv & en,
                Move::CAPTURE,
            );
            list.extend(
                s,
                hv_moves(s, occ) & self.checkmask & self.pin_hv & em,
                Move::QUIET,
            );
            b1 = b1.pop_lsb();
        }

        // HV moves that aren't pinned horizontally
        b1 = self.hv_sliders(state.turn) & !self.pin_d12 & !self.pin_hv;
        while b1 != 0 {
            s = b1.bit_scan();

            list.extend(s, hv_moves(s, occ) & self.checkmask & en, Move::CAPTURE);
            list.extend(s, hv_moves(s, occ) & self.checkmask & em, Move::QUIET);
            b1 = b1.pop_lsb();
        }

        // D12 moves that are pinned diagonally
        b1 = self.d12_sliders(state.turn) & !self.pin_hv & self.pin_d12;
        while b1 != 0 {
            s = b1.bit_scan();
            list.extend(
                s,
                d12_moves(s, occ) & self.checkmask & self.pin_d12 & en,
                Move::CAPTURE,
            );
            list.extend(
                s,
                d12_moves(s, occ) & self.checkmask & self.pin_d12 & em,
                Move::QUIET,
            );
            b1 = b1.pop_lsb();
        }

        // D12 moves that aren't pinned diagonally
        b1 = self.d12_sliders(state.turn) & !self.pin_hv & !self.pin_d12;
        while b1 != 0 {
            s = b1.bit_scan();

            list.extend(s, d12_moves(s, occ) & self.checkmask & en, Move::CAPTURE);
            list.extend(s, d12_moves(s, occ) & self.checkmask & em, Move::QUIET);
            b1 = b1.pop_lsb();
        }

        // Pawn pushes
        b1 = BitBoard::shift_dir(
            self.pieces_bb[state.turn][Piece::PAWN] & !pinned,
            Direction::relative(Direction::North, state.turn),
        ) & em;
        b2 = b1 & !BitBoard::relative_rank(8, state.turn) & self.checkmask;
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::South, state.turn))
                    .bit_scan(),
                s,
                Move::QUIET,
            );
            b2 = b2.pop_lsb();
        }

        // Push Promotions
        b2 = b1 & BitBoard::relative_rank(8, state.turn) & self.checkmask;
        while b2 != 0 {
            s = b2.bit_scan();
            list.add_promotions(
                BitBoard::shift_dir(b2, Direction::relative(Direction::South, state.turn))
                    .bit_scan(),
                s,
                false,
            );
            b2 = b2.pop_lsb();
        }

        // Double pushes
        b2 = BitBoard::shift_dir(b1, Direction::relative(Direction::North, state.turn))
            & em
            & BitBoard::relative_rank(4, state.turn)
            & self.checkmask;
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::South2, state.turn))
                    .bit_scan(),
                s,
                Move::DOUBLE_PUSH,
            );
            b2 = b2.pop_lsb();
        }

        // Pawn pushes (Pin HV)
        b1 = BitBoard::shift_dir(
            self.pieces_bb[state.turn][Piece::PAWN] & !self.pin_d12 & self.pin_hv,
            Direction::relative(Direction::North, state.turn),
        ) & em
            & self.pin_hv
            & self.checkmask;
        b2 = b1;
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::South, state.turn))
                    .bit_scan(),
                s,
                Move::QUIET,
            );
            b2 = b2.pop_lsb();
        }

        // Double pushes (Pin HV)
        b2 = BitBoard::shift_dir(b1, Direction::relative(Direction::North, state.turn))
            & em
            & self.pin_hv
            & BitBoard::relative_rank(4, state.turn)
            & self.checkmask;
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::South2, state.turn))
                    .bit_scan(),
                s,
                Move::DOUBLE_PUSH,
            );
            b2 = b2.pop_lsb();
        }
        /*
               // Pawns that aren't pinned orthogonally
               b1 = self.pieces_bb[state.turn][Piece::PAWN] & !self.pin_hv;
               while b1 != 0 {
                   s = b1.bit_scan();
                   b2 = if self.pin_d12 & b1.get_lsb() != 0 {
                       PAWN_ATTACKS[state.turn][s] & en & self.pin_d12
                   } else {
                       PAWN_ATTACKS[state.turn][s] & en
                   };
                   if b2 & BitBoard::relative_rank(8, state.turn) != 0 {
                       list.extend_promotions(s, b2 & self.checkmask, true);
                   } else {
                       list.extend(s, b2 & self.checkmask, Move::CAPTURE);
                   }
                   if let Some(ep) = state.ep {
                       b2 = PAWN_ATTACKS[state.turn][s] & (1u64 << ep);
                       // If pinned diagonally and the result is over the pinmask
                       if self.pin_d12 & b1.get_lsb() != 0 {
                           list.extend(s, b2 & self.pin_d12 & self.checkmask, Move::EN_PASSANT);
                       } else {
                           // If the en passant ocurrs on the same rank as the king and there is a HV on the same rank, then its ilegal
                           b3 = self.hv_sliders(1 - state.turn) & BitBoard::RANK_1 << (o_king / 8 * 8);
                           if b3 == 0 {
                               if self.checkmask
                                   == BitBoard::shift_dir(
                                       1u64 << ep,
                                       Direction::relative(Direction::South, state.turn),
                                   )
                               {
                                   list.extend(s, b2, Move::EN_PASSANT);
                               } else {
                                   list.extend(s, b2 & self.checkmask, Move::EN_PASSANT);
                               }
                           } else {
                               while b3 != 0 {
                                   if between(b3.bit_scan(), o_king) & occ
                                       != (1u64 << s)
                                           | BitBoard::shift_dir(
                                               1u64 << ep,
                                               Direction::relative(Direction::South, state.turn),
                                           )
                                   {
                                       list.extend(s, b2 & self.checkmask, Move::EN_PASSANT);
                                   }
                                   b3 = b3.pop_lsb();
                               }
                           }
                       }
                   }

                   b1 = b1.pop_lsb();
               }
        */
        // Left captures
        b1 = BitBoard::shift_dir(
            self.pieces_bb[state.turn][Piece::PAWN] & !pinned,
            Direction::relative(Direction::NorthEast, state.turn),
        ) & en
            & self.checkmask;
        b2 = b1 & !BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthWest, state.turn))
                    .bit_scan(),
                s,
                Move::CAPTURE,
            );
            b2 = b2.pop_lsb();
        }
        // Left capture promotions
        b2 = b1 & BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add_promotions(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthWest, state.turn))
                    .bit_scan(),
                s,
                true,
            );
            b2 = b2.pop_lsb();
        }
        // Right captures
        b1 = BitBoard::shift_dir(
            self.pieces_bb[state.turn][Piece::PAWN] & !pinned,
            Direction::relative(Direction::NorthWest, state.turn),
        ) & en
            & self.checkmask;
        b2 = b1 & !BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthEast, state.turn))
                    .bit_scan(),
                s,
                Move::CAPTURE,
            );
            b2 = b2.pop_lsb();
        }
        // Right capture promotions
        b2 = b1 & BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add_promotions(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthEast, state.turn))
                    .bit_scan(),
                s,
                true,
            );
            b2 = b2.pop_lsb();
        }
        // Left captures (Pin D12)
        b1 = BitBoard::shift_dir(
            self.pieces_bb[state.turn][Piece::PAWN] & !self.pin_hv & self.pin_d12,
            Direction::relative(Direction::NorthEast, state.turn),
        ) & en
            & self.pin_d12;
        b2 = b1 & !BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthWest, state.turn))
                    .bit_scan(),
                s,
                Move::CAPTURE,
            );
            b2 = b2.pop_lsb();
        }
        // Left capture promotions (Pin D12)
        b2 = b1 & BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add_promotions(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthWest, state.turn))
                    .bit_scan(),
                s,
                true,
            );
            b2 = b2.pop_lsb();
        }
        // Right captures (Pin D12)
        b1 = BitBoard::shift_dir(
            self.pieces_bb[state.turn][Piece::PAWN] & !self.pin_hv & self.pin_d12,
            Direction::relative(Direction::NorthWest, state.turn),
        ) & en
            & self.pin_d12;
        b2 = b1 & !BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthEast, state.turn))
                    .bit_scan(),
                s,
                Move::CAPTURE,
            );
            b2 = b2.pop_lsb();
        }
        // Right capture promotions (Pin D12)
        b2 = b1 & BitBoard::relative_rank(8, state.turn);
        while b2 != 0 {
            s = b2.bit_scan();
            list.add_promotions(
                BitBoard::shift_dir(b2, Direction::relative(Direction::SouthEast, state.turn))
                    .bit_scan(),
                s,
                true,
            );
            b2 = b2.pop_lsb();
        }

        // En passant
        if let Some(ep) = state.ep {
            b1 = PAWN_ATTACKS[1 - state.turn][ep]
                & self.pieces_bb[state.turn][Piece::PAWN]
                & !self.pin_hv;
            while b1 != 0 {
                b2 = 1u64 << ep;
                // Check if pawn can en passant
                if self.pin_d12 & b1.get_lsb() != 0 {
                    list.extend(s, b2 & self.pin_d12 & self.checkmask, Move::EN_PASSANT);
                } else {
                    // If the en passant ocurrs on the same rank as the king and there is a HV on the same rank, then its ilegal
                    b3 = self.hv_sliders(1 - state.turn) & BitBoard::RANK_1 << (o_king / 8 * 8);
                    if b3 == 0 {
                        if self.checkmask
                            == BitBoard::shift_dir(
                                1u64 << ep,
                                Direction::relative(Direction::South, state.turn),
                            )
                        {
                            list.extend(s, b2, Move::EN_PASSANT);
                        } else {
                            list.extend(s, b2 & self.checkmask, Move::EN_PASSANT);
                        }
                    } else {
                        while b3 != 0 {
                            if between(b3.bit_scan(), o_king) & occ
                                != (1u64 << s)
                                    | BitBoard::shift_dir(
                                        1u64 << ep,
                                        Direction::relative(Direction::South, state.turn),
                                    )
                            {
                                list.extend(s, b2 & self.checkmask, Move::EN_PASSANT);
                            }
                            b3 = b3.pop_lsb();
                        }
                    }
                }
            }
        }

        // Castling is only allowed when:
        // 1. We are not in check
        // 2. The castling area isn't under attack
        // 3. The king and the rook haven't moved
        if self.checkmask == u64::MAX {
            if state.can_castle(State::SHORT[state.turn]) {
                b1 = oo_blockers(state.turn);
                if b1 & !self.danger & !occ == b1 {
                    list.add(o_king, State::SHORT_TARGET[state.turn], Move::CASTLE_00)
                }
            }
            if state.can_castle(State::LONG[state.turn]) {
                b1 = ooo_blockers(state.turn);
                if b1 & (!self.danger | ooo_danger(state.turn)) & !occ == b1 {
                    list.add(
                        o_king,
                        State::LONG_KING_TARGET[state.turn],
                        Move::CASTLE_000,
                    )
                }
            }
        }

        list
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = String::from("  +-----------------+\n");
        for rank in (0..8).rev() {
            s.push_str(&format!("{} | ", rank + 1));
            for file in 0..8 {
                if let Some(piece) = self.piece_on(rank * 8 + file) {
                    let color = self.color_on(rank * 8 + file).unwrap();
                    let mut chr = Piece::to_char(piece);
                    if color == Color::WHITE {
                        chr = chr.to_ascii_uppercase();
                    }
                    s.push_str(&format!("{chr} "));
                } else {
                    s.push_str(". ");
                }
            }
            s.push_str("|\n");
        }
        s.push_str("  +-----------------+\n    a b c d e f g h");
        write!(f, "{s}")
    }
}

impl FromStr for Position {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pos = Position::new();
        let params = s.split(" ").collect::<Vec<&str>>();
        let ranks = params[0].split("/").collect::<Vec<&str>>();

        let mut sq = 0;

        for rank in ranks.iter().rev() {
            for piece in rank.chars() {
                if piece.is_digit(10) {
                    sq += piece.to_digit(10).unwrap();
                    continue;
                }
                let color = if piece.is_uppercase() { 0 } else { 1 };
                let piece_type = Piece::from_char(piece);
                pos.pieces_bb[color][piece_type] |= 1u64 << sq;
                sq += 1;
            }
        }
        if pos.pieces_bb[Color::WHITE][Piece::KING] == 0
            || pos.pieces_bb[Color::BLACK][Piece::KING] == 0
        {
            panic!("One king is missing")
        }
        pos.history[pos.ply].turn = if params[1] == "w" {
            Color::WHITE
        } else {
            Color::BLACK
        };

        pos.history[pos.ply].castling = 0;
        if params[2].contains("K") {
            pos.history[pos.ply].castling |= State::WHITE_00
        }
        if params[2].contains("Q") {
            pos.history[pos.ply].castling |= State::WHITE_000
        }
        if params[2].contains("k") {
            pos.history[pos.ply].castling |= State::BLACK_00
        }
        if params[2].contains("q") {
            pos.history[pos.ply].castling |= State::BLACK_000
        }

        if params[3] != "-" {
            pos.history[pos.ply].ep = Some(Square::from_str(params[3]))
        }

        if params[4] != "-" {
            pos.history[pos.ply].hm = params[4].parse::<usize>().unwrap()
        }
        if params[5] != "-" {
            pos.history[pos.ply].fm = params[5].parse::<usize>().unwrap()
        }

        let (c, hv, d12) = pos.check_and_pin();
        pos.checkmask = c;
        pos.pin_hv = hv;
        pos.pin_d12 = d12;
        pos.danger = pos.attacks();

        Ok(pos)
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::from_str("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap()
    }
}
