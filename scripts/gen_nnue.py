import numpy as np

# 100 cases * 3 types de pièces * 2 camps = 600 caractéristiques binaires
FEATURE_COUNT = 600
ACCUMULATOR_SIZE = 64
SCALE = 128

def main():
    np.random.seed(42)
    # Initialisation des poids d'accumulation (Features -> Accumulateur)
    # Poids calibrés pour favoriser l'avancée de la Sentinelle et le contrôle des crêtes
    feature_weights = np.random.randint(-15, 15, size=(FEATURE_COUNT, ACCUMULATOR_SIZE), dtype=np.int16)

    for sq in range(100):
        y = sq // 10
        # Sentinelle alliée camp 1 (avance vers le bas y=9)
        feat_sent_c1 = (sq * 6) + 4
        feature_weights[feat_sent_c1] += int(y * 8)

        # Sentinelle alliée camp 2 (avance vers le haut y=0)
        feat_sent_c2 = (sq * 6) + 5
        feature_weights[feat_sent_c2] += int((9 - y) * 8)

    # Couche de sortie (Accumulateur -> Score d'évaluation)
    output_weights = np.random.randint(-30, 30, size=ACCUMULATOR_SIZE, dtype=np.int16)
    output_bias = 0

    print("Génération de src/nnue_weights.rs...")
    with open("src/nnue_weights.rs", "w") as f:
        f.write("// Poids quantifiés entiers pour moteur NNUE\n")
        f.write(f"pub const FEATURE_COUNT: usize = {FEATURE_COUNT};\n")
        f.write(f"pub const ACCUMULATOR_SIZE: usize = {ACCUMULATOR_SIZE};\n\n")

        f.write("pub static FEATURE_WEIGHTS: [[i16; ACCUMULATOR_SIZE]; FEATURE_COUNT] = [\n")
        for row in feature_weights:
            vals = ", ".join(map(str, row))
            f.write(f"    [{vals}],\n")
        f.write("];\n\n")

        f.write("pub static OUTPUT_WEIGHTS: [i16; ACCUMULATOR_SIZE] = [\n    ")
        f.write(", ".join(map(str, output_weights)))
        f.write("\n];\n")
        f.write(f"pub const OUTPUT_BIAS: i32 = {output_bias};\n")

    print("src/nnue_weights.rs prêt !")

if __name__ == "__main__":
    main()
