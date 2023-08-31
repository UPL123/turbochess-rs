# TurboChess library

TurboChess is a chess move generator. It supports:

* PEXT bitboards (emulated)
* Make and Undo Position
* Zobrist hashing
* FEN support

To get started, create a new [Position](https://docs.rs/turbochess/0.1.0/turbochess/struct.Position.html) and now you can work with legal moves

## Example 1: Get the count of legal moves

```rs
let pos: Position = Position::default(); // Loads the initial position
// Or you can also use another position
// let pos: Position = Position::from_str("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();

let legals: MoveList = pos.legal(); // Gets all legal moves in the position
for mv in legals {
   println!("{mv}"); // Prints the move in the UCI format
}

println!("Legal count: {}", legals.count());
```

## Example 2: Test all legal moves

```rs
let mut pos: Position = Position::default();
let legals: MoveList = pos.legal(); // Gets all legal moves in the position
for mv in legals {
   pos.make_move(mv);
   println!("Doing stuff with move...")
   pos.undo_move(mv);
}
println!("Finished!")
```

## Example 3: Play a random move

```rs
let mut pos: Position = Position::default();
let legals: MoveList = pos.legal();

// Gets a random move
let mut rng = rand::thread_rng();
let index = rng.gen_range(0..legals.count());
let mv = legals.get(index);

println!("Move: {mv}");
pos.make_move(mv);
println!("{pos}");
println!("New FEN: {}", pos.fen());
```

## Contribute to TurboChess

Actually, TurboChess probably has performance issues and errors that can be fixed to be faster and accurate. If you find some of these errors, please raise an issue. Thanks!
