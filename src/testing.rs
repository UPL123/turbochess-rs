use crate::{
    lookup::{line, D12_MASKS, HV_MASKS},
    types::{Move, MoveList, Square},
    Position,
};
use std::str::FromStr;
use std::time::Instant;
extern crate term_table;

pub fn perft(mut pos: Position, depth: usize) -> i64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal();
    let mut nodes = 0;
    for mv in moves {
        pos.make_move(mv);
        nodes += perft(pos, depth - 1);
        pos.undo_move(mv);
    }
    nodes
}

pub fn perft_complete(
    mut pos: Position,
    depth: usize,
    cps: &mut usize,
    eps: &mut usize,
    cast: &mut usize,
    proms: &mut usize,
    checks: &mut usize,
    mates: &mut usize,
) -> i64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal();
    let mut nodes = 0;
    *cps += moves.count_captures();
    *eps += moves.count_enpassants();
    *cast += moves.count_castles();
    *proms += moves.count_promotions();
    if pos.in_check() {
        *checks += 1;
        if moves.clone().count() == 0 {
            *mates += 1;
        }
    }
    for mv in moves {
        pos.make_move(mv);
        nodes += perft_complete(pos, depth - 1, cps, eps, cast, proms, checks, mates);
        pos.undo_move(mv);
    }
    nodes
}

pub fn perft_divide(mut pos: Position, depth: usize) -> i64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal();
    let mut nodes = 0;
    for mv in moves {
        pos.make_move(mv);
        let res = perft(pos, depth - 1);
        println!("{mv}: {res}");
        nodes += res;
        pos.undo_move(mv);
    }
    nodes
}

macro_rules! test_perft {
    ($fen:expr, $depth:expr, $expected:expr) => {
        let pos = Position::from_str($fen).unwrap();
        let start = Instant::now();
        let nodes = perft(pos, $depth);
        let duration = start.elapsed();
        let nps = nodes as f64 / duration.as_secs_f64();
        print!(
            "[PERFT] Fen = '{}'; Depth = {}; NPS = {}; Result = ",
            $fen, $depth, nps
        );
        if nodes == $expected {
            println!("OK");
        } else {
            println!("FAIL");
        }
    };
}

#[test]
fn test_movegen() {
    let pos = Position::from_str("B3n1N1/b3P1PK/R1P1P3/7R/4p3/8/7Q/6k1 b - - 0 2").unwrap();
    println!("{pos}");
    let legal = pos.legal();
    for mv in legal {
        println!("{mv}")
    }
}

#[test]
fn lookup_test() {
    let bb = line(6, 15) & D12_MASKS[6];
    println!("{bb}");
}
#[test]
fn perft_test() {
    let mut pos =
        Position::from_str("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
            .unwrap();

    let depth = 4;
    let start = Instant::now();
    let nodes = perft_divide(pos, depth);
    let duration = start.elapsed();
    let nps = nodes as f64 / duration.as_secs_f64();
    println!(
        "
Nodes searched: {nodes}
NPS: {nps}
KNPS: {},
MNPS: {}",
        nps / 1000.,
        nps / 1000000.
    )
}

#[test]
fn perft_tests() {
    // Start position
    test_perft!(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        3,
        8902
    );
    test_perft!(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        4,
        197281
    );
    test_perft!(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        5,
        4865609
    );
    test_perft!(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        6,
        119060324
    );
    test_perft!(
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        7,
        3195901860
    );
    // Kiwipete
    test_perft!(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        2,
        2039
    );
    test_perft!(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        3,
        97862
    );
    test_perft!(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        4,
        4085603
    );
    test_perft!(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        5,
        193690690
    );
    test_perft!(
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        6,
        8031647685
    );
    // Position 5
    test_perft!(
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        3,
        62379
    );
    test_perft!(
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        4,
        2103487
    );
    test_perft!(
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        5,
        89941194
    );
}
