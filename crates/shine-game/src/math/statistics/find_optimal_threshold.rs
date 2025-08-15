/// Finds the threshold that maximizes the F-score (with the given beta) for separating positive and negative scores.
/// Higher beta favors recall; lower beta favors precision.
pub fn find_optimal_threshold(positive_scores: &[f32], negative_scores: &[f32], beta: f32) -> f32 {
    assert!(
        !positive_scores.is_empty() && !negative_scores.is_empty(),
        "Scores must not be empty"
    );

    // Sort the scores
    let mut all_scores = Vec::with_capacity(positive_scores.len() + negative_scores.len());
    all_scores.extend(positive_scores.iter().map(|&s| (s, true)));
    all_scores.extend(negative_scores.iter().map(|&s| (s, false)));
    all_scores.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let beta2 = beta * beta;
    let mut true_positive = 0.0;
    let mut false_positive = 0.0;
    let mut false_negative = positive_scores.len() as f32;

    let mut best_f_beta = 0.0;
    let mut best_threshold = f32::NEG_INFINITY;

    for (score, label) in all_scores {
        if label {
            true_positive += 1.0;
            false_negative -= 1.0;
        } else {
            false_positive += 1.0;
        }

        let precision = if true_positive + false_positive > 0.0 {
            true_positive / (true_positive + false_positive)
        } else {
            0.0
        };
        let recall = if true_positive + false_negative > 0.0 {
            true_positive / (true_positive + false_negative)
        } else {
            0.0
        };

        let f_beta = if precision + recall > 0.0 {
            (1.0 + beta2) * precision * recall / (beta2 * precision + recall)
        } else {
            0.0
        };

        if f_beta > best_f_beta {
            best_f_beta = f_beta;
            best_threshold = score;
        }
    }

    best_threshold
}
