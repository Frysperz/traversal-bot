import json
import numpy as np

FEATURE_COUNT = 600
ACCUMULATOR_SIZE = 128
SCALE = 16

def relu_clipped(x):
    return np.clip(x, 0, 127)

def main():
    print("Chargement du dataset de self-play...")
    features_list = []
    targets = []

    with open("dataset.jsonl", "r") as f:
        for line in f:
            data = json.loads(line)
            features_list.append(data["features"])
            targets.append(data["result"] * 100.0) # Score en centipions (-100 à +100)

    n_samples = len(targets)
    print(f"{n_samples} positions extraites pour l'entraînement.")

    # Poids initiaux
    np.random.seed(42)
    feature_weights = np.random.uniform(-4.0, 4.0, size=(FEATURE_COUNT, ACCUMULATOR_SIZE)).astype(np.float32)
    output_weights = np.random.uniform(-1.0, 1.0, size=ACCUMULATOR_SIZE).astype(np.float32)
    output_bias = 0.0

    lr_feature = 0.005
    lr_output = 0.002
    epochs = 20
    batch_size = 256

    print("Entraînement de l'architecture NNUE...")
    indices = np.arange(n_samples)

    for epoch in range(epochs):
        np.random.shuffle(indices)
        total_loss = 0.0

        for start in range(0, n_samples, batch_size):
            b_idx = indices[start:start+batch_size]
            b_targets = np.array([targets[i] for i in b_idx], dtype=np.float32)

            # Forward pass avec features éparses
            accumulators = np.zeros((len(b_idx), ACCUMULATOR_SIZE), dtype=np.float32)
            for row_i, data_i in enumerate(b_idx):
                for f_id in features_list[data_i]:
                    accumulators[row_i] += feature_weights[f_id]

            activated = relu_clipped(accumulators)
            preds = (np.dot(activated, output_weights) + output_bias) / SCALE

            errors = preds - b_targets
            loss = np.mean(errors ** 2)
            total_loss += loss * len(b_idx)

            # Gradient descent
            d_preds = 2.0 * errors / (len(b_idx) * SCALE)
            d_output = np.dot(d_preds, activated)
            output_weights -= lr_output * d_output

            d_activated = np.outer(d_preds, output_weights)
            d_accumulators = d_activated * ((accumulators > 0) & (accumulators < 127)).astype(np.float32)

            for row_i, data_i in enumerate(b_idx):
                for f_id in features_list[data_i]:
                    feature_weights[f_id] -= lr_feature * d_accumulators[row_i]

        print(f"Époque {epoch+1:02d}/{epochs} - Perte MSE: {total_loss / n_samples:.2f}")

    # Quantification en entiers i16
    quantized_feature_weights = np.clip(np.round(feature_weights), -128, 127).astype(np.int16)
    quantized_output_weights = np.clip(np.round(output_weights * 4.0), -64, 63).astype(np.int16)

    print("Écriture des poids quantifiés dans src/nnue_weights.rs...")
    with open("src/nnue_weights.rs", "w") as f:
        f.write("// Poids NNUE quantifiés issus du pipeline de Self-Play\n")
        f.write("use crate::nnue::{ACCUMULATOR_SIZE, FEATURE_COUNT};\n\n")

        f.write("pub static TRAINED_FEATURE_WEIGHTS: [[i16; ACCUMULATOR_SIZE]; FEATURE_COUNT] = [\n")
        for row in quantized_feature_weights:
            f.write(f"    [{', '.join(map(str, row))}],\n")
        f.write("];\n\n")

        f.write("pub static TRAINED_OUTPUT_WEIGHTS: [i16; ACCUMULATOR_SIZE] = [\n    ")
        f.write(", ".join(map(str, quantized_output_weights)))
        f.write("\n];\n")
        f.write(f"pub const TRAINED_OUTPUT_BIAS: i32 = {int(round(output_bias))};\n")

    print("src/nnue_weights.rs généré avec succès !")

if __name__ == "__main__":
    main()
