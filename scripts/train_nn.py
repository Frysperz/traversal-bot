import numpy as np

# Architecture étendue : 110 -> 256 -> 1
INPUT_SIZE = 110
HIDDEN_SIZE = 256
OUTPUT_SIZE = 1

def relu(x):
    return np.maximum(0, x)

def relu_derivative(x):
    return (x > 0).astype(np.float32)

def generate_tactical_dataset(num_samples=50000):
    print(f"Génération du dataset élargi ({num_samples} positions)...")
    X = np.zeros((num_samples, INPUT_SIZE), dtype=np.float32)
    Y = np.zeros((num_samples, OUTPUT_SIZE), dtype=np.float32)

    for i in range(num_samples):
        # 0..99 : état du plateau
        friendly_cases = np.random.choice(100, 6, replace=False)
        enemy_cases = np.random.choice([c for c in range(100) if c not in friendly_cases], 6, replace=False)
        for c in friendly_cases:
            X[i, c] = 1.0
        for c in enemy_cases:
            X[i, c] = -1.0

        # Features du coup
        to_case = np.random.randint(0, 100)
        target_y = to_case // 10
        piece_kind = np.random.choice([0, 1, 2]) # 0: Exp, 1: Vang, 2: Sent
        kind_move = np.random.choice([0, 1, 2])
        attack = np.random.randint(1, 6)
        defense = np.random.randint(1, 6)
        cost = np.random.randint(1, 5)
        height = np.random.randint(0, 5)

        X[i, 100] = float(target_y) / 9.0
        X[i, 101] = float(piece_kind) / 2.0
        X[i, 102] = 1.0 if kind_move == 1 else 0.0
        X[i, 103] = 1.0 if kind_move == 2 else 0.0
        X[i, 104] = float(attack) / 5.0
        X[i, 105] = float(defense) / 5.0
        X[i, 106] = float(cost) / 5.0
        X[i, 107] = float(height) / 4.0
        X[i, 108] = 1.0 if (kind_move == 2 and piece_kind == 2) else 0.0
        X[i, 109] = float(attack - defense) / 5.0

        # Heuristique d'entraînement de grand-maître
        score = 0.0
        if kind_move == 2:
            score = 3000.0 if piece_kind == 2 else 1000.0
        elif kind_move == 1:
            diff = attack - defense
            if diff >= 0:
                score = 250.0 + diff * 60.0 + height * 25.0
            else:
                score = -300.0 # Pénalité fatale pour un sacrifice raté
        else:
            prog = target_y
            if piece_kind == 2:
                score = prog * 50.0 + height * 25.0 - cost * 6.0
            elif piece_kind == 1:
                score = prog * 30.0 + height * 18.0 - cost * 5.0
            else:
                score = prog * 15.0 - cost * 3.0

        Y[i, 0] = score / 100.0

    return X, Y

def train():
    np.random.seed(42)
    X, Y = generate_tactical_dataset(50000)

    # Initialisation He
    w1 = np.random.randn(INPUT_SIZE, HIDDEN_SIZE).astype(np.float32) * np.sqrt(2.0 / INPUT_SIZE)
    b1 = np.zeros(HIDDEN_SIZE, dtype=np.float32)
    w2 = np.random.randn(HIDDEN_SIZE, OUTPUT_SIZE).astype(np.float32) * np.sqrt(2.0 / HIDDEN_SIZE)
    b2 = np.zeros(OUTPUT_SIZE, dtype=np.float32)

    lr = 0.002
    batch_size = 256
    epochs = 40

    print(f"Entraînement accéléré ({epochs} époques, {HIDDEN_SIZE} neurones)...")
    num_samples = X.shape[0]

    for epoch in range(epochs):
        indices = np.arange(num_samples)
        np.random.shuffle(indices)
        epoch_loss = 0.0

        for start in range(0, num_samples, batch_size):
            b_idx = indices[start:start+batch_size]
            xb, yb = X[b_idx], Y[b_idx]

            z1 = np.dot(xb, w1) + b1
            a1 = relu(z1)
            preds = np.dot(a1, w2) + b2

            loss = np.mean((preds - yb) ** 2)
            epoch_loss += loss * len(b_idx)

            d_out = 2.0 * (preds - yb) / len(b_idx)
            dw2 = np.dot(a1.T, d_out)
            db2 = np.sum(d_out, axis=0)

            da1 = np.dot(d_out, w2.T)
            dz1 = da1 * relu_derivative(z1)
            dw1 = np.dot(xb.T, dz1)
            db1 = np.sum(dz1, axis=0)

            w1 -= lr * dw1
            b1 -= lr * db1
            w2 -= lr * dw2
            b2 -= lr * db2

        if (epoch + 1) % 10 == 0:
            print(f"Époque {epoch+1:02d}/{epochs} - Perte MSE: {epoch_loss / num_samples:.4f}")

    print("Exportation vers src/weights.rs...")
    with open("src/weights.rs", "w") as f:
        f.write("// Poids entraînés sur 50 000 positions\n")
        f.write(f"pub const INPUT_SIZE: usize = {INPUT_SIZE};\n")
        f.write(f"pub const HIDDEN_SIZE: usize = {HIDDEN_SIZE};\n\n")

        f.write("pub static W1: [[f32; HIDDEN_SIZE]; INPUT_SIZE] = [\n")
        for row in w1:
            vals = ", ".join(f"{v:.5f}f32" for v in row)
            f.write(f"    [{vals}],\n")
        f.write("];\n\n")

        vals_b1 = ", ".join(f"{v:.5f}f32" for v in b1)
        f.write(f"pub static B1: [f32; HIDDEN_SIZE] = [{vals_b1}];\n\n")

        vals_w2 = ", ".join(f"{v[0]:.5f}f32" for v in w2)
        f.write(f"pub static W2: [f32; HIDDEN_SIZE] = [{vals_w2}];\n\n")

        f.write(f"pub const B2: f32 = {b2[0]:.5f}f32;\n")

    print("Exportation terminée !")

if __name__ == "__main__":
    train()
