use std::{str::FromStr, thread};

use clap::{arg, Args, Parser, Subcommand};
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    Table, TableStyle,
};
use turbochess::{
    testing::{perft, perft_complete, perft_divide},
    Position,
};

#[derive(Args)]
pub struct PerftOptions {
    /// the FEN notation of the position
    #[arg(
        short,
        long,
        default_value_t = String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    )]
    fen: String,
    /// the depth to search to
    #[arg(short, long, default_value_t = 3)]
    depth: usize,
}

#[derive(Args)]
pub struct ListOptions {
    /// the FEN notation of the position
    #[arg(
        short,
        long,
        default_value_t = String::from("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    )]
    fen: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Runs a perft for a custom position at a custom depth
    Perft(PerftOptions),
    /// Runs a divide perft for a custom position at a custom depth
    Divide(PerftOptions),
    /// Lists all possible moves in the position
    List(PerftOptions),
    /// Returns a complete perft including checks, captures, promotions, en passants and checkmates
    Complete(PerftOptions),
}

#[derive(Parser)]
#[command(author = "UPL", version = "1.0.0", about = "A fast move generator, situable for chess engines", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

fn main() {
    let child = thread::Builder::new()
        .stack_size(1046 * 64 * 64)
        .spawn(move || {
            let cli = Cli::parse();
            match &cli.command {
                Commands::Perft(options) => {
                    let pos = Position::from_str(&options.fen).expect("Invalid FEN");
                    let (checkmask, pin_hv, pin_d12) = pos.check_and_pin();
                    println!("{pos}");
                    println!("FEN: {}", pos.fen());
                    println!("Checkmask: {checkmask}");
                    println!("Pinned: {}\n", pin_hv | pin_d12);
                    for depth in 0..=options.depth {
                        let nodes = perft(pos, depth);
                        println!("Perft {depth}: {nodes}")
                    }
                }
                Commands::Divide(options) => {
                    let pos = Position::from_str(&options.fen).expect("Invalid FEN");
                    let (checkmask, pin_hv, pin_d12) = pos.check_and_pin();
                    println!("{pos}");
                    println!("FEN: {}", options.fen);
                    println!("Checkmask: {checkmask}");
                    println!("Pinned: {}\n", pin_hv | pin_d12);
                    let nodes = perft_divide(pos, options.depth);
                    println!("\nNodes searched: {nodes}")
                }
                Commands::List(options) => {
                    let pos = Position::from_str(&options.fen).expect("Invalid FEN");
                    let (checkmask, pin_hv, pin_d12) = pos.check_and_pin();
                    println!("{pos}");
                    println!("FEN: {}", pos.fen());
                    println!("Checkmask: {checkmask}");
                    println!("Pinned: {}\n", pin_hv | pin_d12);
                    let list = pos.legal();
                    println!("=== LIST START ===");
                    for mv in list {
                        println!("{mv}")
                    }
                    println!("==================");
                    println!("\nLegal moves: {}", list.count());
                }
                Commands::Complete(options) => {
                    let pos = Position::from_str(&options.fen).expect("Invalid FEN");
                    let (checkmask, pin_hv, pin_d12) = pos.check_and_pin();
                    println!("{pos}");
                    println!("FEN: {}", pos.fen());
                    println!("Checkmask: {checkmask}");
                    println!("Pinned: {}\n", pin_hv | pin_d12);
                    let mut table = Table::new();
                    table.style = TableStyle::elegant();

                    table.add_row(Row::new(vec![TableCell::new_with_col_span(
                        "Perft results",
                        8,
                    )]));
                    table.add_row(Row::new(vec![
                        TableCell::new("Depth"),
                        TableCell::new("Nodes"),
                        TableCell::new("Captures"),
                        TableCell::new("En Passants"),
                        TableCell::new("Castles"),
                        TableCell::new("Promotions"),
                        TableCell::new("Checks"),
                        TableCell::new("Checkmates"),
                    ]));
                    for depth in 0..=options.depth {
                        let mut cps = 0;
                        let mut eps = 0;
                        let mut cast = 0;
                        let mut proms = 0;
                        let mut checks = 0;
                        let mut mates = 0;
                        let nodes = perft_complete(
                            pos,
                            depth,
                            &mut cps,
                            &mut eps,
                            &mut cast,
                            &mut proms,
                            &mut checks,
                            &mut mates,
                        );
                        let mut row = Row::new(vec![
                            TableCell::new(depth.to_string()),
                            TableCell::new(nodes.to_string()),
                            TableCell::new(cps.to_string()),
                            TableCell::new(eps.to_string()),
                            TableCell::new(cast.to_string()),
                            TableCell::new(proms.to_string()),
                            TableCell::new(checks.to_string()),
                            TableCell::new(mates.to_string()),
                        ]);
                        table.add_row(row);
                        println!("[DONE] Depth {depth}")
                    }
                    println!("{}", table.render())
                }
            }
        })
        .unwrap();
    child.join().unwrap()
}
