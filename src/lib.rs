use traversal_bot::core::MoveKind;
use traversal_bot::{bot, Turn};

// turn.state : la position, turn.moves : les coups légaux, turn.seed : le hasard.
fn choose(turn: &Turn) -> usize {
    // Joue une sortie si possible, sinon le premier coup légal disponible.
    turn.moves.iter().position(|m| m.kind == MoveKind::Exit).unwrap_or(0)
}

bot!(choose);
