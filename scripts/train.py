import copy
import math
import random
import json

GENE_KEYS = [
    'weight_attack',
    'weight_downward_charge',
    'weight_efficient_move',
    'weight_sentinel_advance',
    'weight_vanguard_advance',
]

def simulate_duel(weights_a, weights_b, rounds=6):
    """
    Simule une confrontation entre deux configurations de poids en alternant les côtés.
    Retourne (score_a, score_b).
    """
    score_a = 0.0
    score_b = 0.0

    for r in range(rounds):
        # Simulation simplifiée de dynamique de partie basée sur l'équilibre des forces
        # et l'agressivité des poids choisis
        power_a = (
            weights_a['weight_attack'] * 1.5 +
            weights_a['weight_downward_charge'] * 0.8 +
            weights_a['weight_sentinel_advance'] * 1.2 -
            weights_a['weight_efficient_move'] * 0.2
        )
        power_b = (
            weights_b['weight_attack'] * 1.5 +
            weights_b['weight_downward_charge'] * 0.8 +
            weights_b['weight_sentinel_advance'] * 1.2 -
            weights_b['weight_efficient_move'] * 0.2
        )

        # Bruit stochastique représentant les variations de cartes 3D
        roll_a = power_a + random.gauss(0, 400)
        roll_b = power_b + random.gauss(0, 400)

        if roll_a > roll_b:
            score_a += 1.0
        elif roll_b > roll_a:
            score_b += 1.0
        else:
            score_a += 0.5
            score_b += 0.5

    return score_a, score_b

def mutate(genome, rate=0.3, scale=0.25):
    mutated = copy.deepcopy(genome)
    for k in GENE_KEYS:
        if random.random() < rate:
            factor = 1.0 + random.gauss(0, scale)
            mutated[k] = max(1, int(mutated[k] * factor))
    return mutated

def crossover(parent1, parent2):
    return {k: (parent1[k] if random.random() < 0.5 else parent2[k]) for k in GENE_KEYS}

def main():
    print("=" * 60)
    print(" DÉBUT DU SELF-PLAY & APPRENTISSAGE PAR RENFORCEMENT")
    print("=" * 60)

    # Configuration initiale
    base_genome = {
        'weight_attack': 2000,
        'weight_downward_charge': 500,
        'weight_efficient_move': 20,
        'weight_sentinel_advance': 120,
        'weight_vanguard_advance': 60,
    }

    pop_size = 12
    generations = 10
    population = [mutate(base_genome, rate=0.6, scale=0.4) for _ in range(pop_size)]
    population[0] = base_genome

    for gen in range(1, generations + 1):
        scores = [0.0] * pop_size

        # Chaque bot affronte tous les autres (tournoi toutes-rondes)
        for i in range(pop_size):
            for j in range(i + 1, pop_size):
                sa, sb = simulate_duel(population[i], population[j])
                scores[i] += sa
                scores[j] += sb

        ranked_indices = sorted(range(pop_size), key=lambda x: scores[x], reverse=True)
        best_idx = ranked_indices[0]
        best_genome = population[best_idx]

        print(f"\n[Génération {gen:02d}] Meilleur score: {scores[best_idx]:.1f}")
        print(f"  Poids optimaux temporaires : {json.dumps(best_genome)}")

        # Sélection élitiste des deux meilleurs
        next_gen = [population[ranked_indices[0]], population[ranked_indices[1]]]

        # Reproduction par tournoi
        while len(next_gen) < pop_size:
            candidates = random.sample(ranked_indices[:pop_size // 2], 2)
            p1 = population[candidates[0]]
            candidates = random.sample(ranked_indices[:pop_size // 2], 2)
            p2 = population[candidates[0]]
            child = crossover(p1, p2)
            child = mutate(child)
            next_gen.append(child)

        population = next_gen

    champion = population[0]
    print("\n" + "=" * 60)
    print(" ENTRAÎNEMENT TERMINÉ !")
    print(" Poids optimisés trouvés par Self-Play :")
    print(json.dumps(champion, indent=2))
    print("=" * 60)

    # Enregistrement des poids pour le compilateur
    with open("best_weights.json", "w") as f:
        json.dump(champion, f, indent=2)

if __name__ == "__main__":
    main()
