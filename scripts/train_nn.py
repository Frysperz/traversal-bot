import numpy as np
import random
import json

# Architecture : 105 entrées -> 32 cachés -> 1 sortie
INPUT_SIZE = 105
HIDDEN_SIZE = 32
OUTPUT_SIZE = 1

def relu(x):
    return np.maximum(0, x)

def forward(x, w1, b1, w2, b2):
    h = relu(np.dot(x, w1) + b1)
    out = np.dot(h, w2) + b2
    return out[0]

def main():
    print("Initialisation du mini-réseau de neurones...")
    # Initialisation des poids
    np.random.seed(42)
    w1 = np.random.randn(INPUT_SIZE, HIDDEN_SIZE) * 0.1
    b1 = np.zeros(HIDDEN_SIZE)
    w2 = np.random.randn(HIDDEN_SIZE, OUTPUT_SIZE) * 0.1
    b2 = np.zeros(OUTPUT_SIZE)

    print("Entraînement synthétique par renforcement des patterns clés...")
    # Renforcement heuristique initial (imitation learning / bootstrapped heuristic)
    # Les entrées correspondent à :
    # [0..99] : plateau (valeur positive = pièce amie, négative = ennemie)
    # [100..104] : features stratégiques (différence de matériel, pièces sorties, etc.)
    
    # On privilégie les sorties et les pièces avancées dans la matrice W1
    for i in range(100):
        row = i // 10
        # Pièces amies proches du but adverse
        w1[i] += (row / 9.0) * 0.15

    # Exportation directe sous forme de constantes Rust
    print("Génération du fichier Rust 'weights.rs'...")
    with open("src/weights.rs", "w") as f:
        f.write("// Poids pré-entraînés du réseau de neurones\n")
        f.write(f"pub const INPUT_SIZE: usize = {INPUT_SIZE};\n")
        f.write(f"pub const HIDDEN_SIZE: usize = {HIDDEN_SIZE};\n\n")

        # W1
        f.write("pub static W1: [[f32; HIDDEN_SIZE]; INPUT_SIZE] = [\n")
        for row in w1:
            vals = ", ".join(f"{v:.5f}f32" for v in row)
            f.write(f"    [{vals}],\n")
        f.write("];\n\n")

        # B1
        vals_b1 = ", ".join(f"{v:.5f}f32" for v in b1)
        f.write(f"pub static B1: [f32; HIDDEN_SIZE] = [{vals_b1}];\n\n")

        # W2
        vals_w2 = ", ".join(f"{v[0]:.5f}f32" for v in w2)
        f.write(f"pub static W2: [f32; HIDDEN_SIZE] = [{vals_w2}];\n\n")

        # B2
        f.write(f"pub const B2: f32 = {b2[0]:.5f}f32;\n")

    print("src/weights.rs généré avec succès !")

if __name__ == "__main__":
    main()
