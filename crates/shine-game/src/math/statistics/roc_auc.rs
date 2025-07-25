/// Measure the classification performance of a binary classifier using the ROC AUC metric.
/// The function returns the quality of the classifier:
/// - score ~0.9, indicating a good classifier
/// - score ~0.5, indicating a random classifier
/// - score < 0.5, indicating a bad classifier with inverted predictions
pub fn roc_auc(positive_scores: &[f32], negative_scores: &[f32]) -> f32 {
    assert!(
        !positive_scores.is_empty() && !negative_scores.is_empty(),
        "Scores must not be empty"
    );

    // Sort the scores
    let mut all_scores = Vec::with_capacity(positive_scores.len() + negative_scores.len());
    all_scores.extend(positive_scores.iter().map(|&s| (s, true)));
    all_scores.extend(negative_scores.iter().map(|&s| (s, false)));
    all_scores.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let count = all_scores.len();
    let all_positives = positive_scores.len() as f32;
    let all_negative = negative_scores.len() as f32;

    // construct roc curve
    // ROC Graphs: Notes and Practical Considerations for Researchers - Tom Fawcett (Algorithm 2)
    let roc = {
        let mut roc = Vec::with_capacity(count);
        let mut f_prev = f32::INFINITY;
        let mut true_positives = 0.0;
        let mut false_positives = 0.0;
        for (f_i, label) in all_scores.into_iter() {
            if f_i != f_prev {
                roc.push((true_positives / all_positives, false_positives / all_negative));
                f_prev = f_i;
            }
            if label {
                true_positives += 1.0;
            } else {
                false_positives += 1.0;
            }
        }
        roc.push((true_positives / all_positives, false_positives / all_negative));
        roc
    };

    // Area under the ROC curve (AUC)
    let mut auc = 0.0;
    for i in 1..roc.len() {
        let (x1, y1) = roc[i - 1];
        let (x2, y2) = roc[i];
        auc += (x2 - x1) * (y2 + y1) / 2.0;
    }
    auc
}
