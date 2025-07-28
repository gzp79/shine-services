use core::f32;
use std::io;

/// Scoring mode for the ROC AUC calculation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScoringMode {
    /// Higher scores indicate better predictions (typical confidence scores)
    HigherIsBetter,
    /// Lower scores indicate better predictions (e.g., distance/error metrics)
    LowerIsBetter,
}

/// Find the ROC curve and return the TPR and FPR value pairs.
/// Based on 'ROC Graphs: Notes and Practical Considerations for Researchers - Tom Fawcett (Algorithm 2)'
pub fn roc(positive_scores: &[f32], negative_scores: &[f32], mode: ScoringMode) -> Vec<(f32, f32)> {
    assert!(
        !positive_scores.is_empty() && !negative_scores.is_empty(),
        "Scores must not be empty"
    );

    // Sort the scores
    let mut all_scores = Vec::with_capacity(positive_scores.len() + negative_scores.len());
    all_scores.extend(positive_scores.iter().map(|&s| (s, true)));
    all_scores.extend(negative_scores.iter().map(|&s| (s, false)));

    match mode {
        ScoringMode::HigherIsBetter => all_scores.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap()),
        ScoringMode::LowerIsBetter => all_scores.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap()),
    }

    let count = all_scores.len();
    let all_positives = positive_scores.len() as f32;
    let all_negative = negative_scores.len() as f32;

    let mut roc = Vec::with_capacity(count);
    let mut f_prev = f32::NAN;
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
}

pub fn dump_roc(roc: &[(f32, f32)], file: &mut dyn io::Write) -> io::Result<()> {
    writeln!(file, "fpr,tpr")?;
    for (tpr, fpr) in roc {
        writeln!(file, "{},{}", fpr, tpr)?;
    }
    Ok(())
}

// Find the area under the ROC curve
pub fn auc(roc: &[(f32, f32)]) -> f32 {
    // Area under the ROC curve (AUC)
    let mut auc = 0.0;
    for i in 1..roc.len() {
        let (tpr1, fpr1) = roc[i - 1];
        let (tpr2, fpr2) = roc[i];
        let w = fpr2 - fpr1;
        let h = (tpr1 + tpr2) / 2.0;
        auc += w * h;
    }
    auc
}

/// Measure the classification performance of a binary classifier using the ROC AUC metric.
/// The function returns the quality of the classifier:
/// - score ~0.9, indicating a good classifier
/// - score ~0.5, indicating a random classifier
/// - score < 0.5, indicating a bad classifier with inverted predictions
pub fn roc_auc(positive_scores: &[f32], negative_scores: &[f32], mode: ScoringMode) -> f32 {
    let roc = roc(positive_scores, negative_scores, mode);
    auc(&roc)
}
