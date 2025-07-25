use std::{
    fs::File,
    io::{self, Write},
};

/// Dump positive and negative scores to CSV file
pub fn dump_classification_scores_to_csv(
    filename: &str,
    positive_scores: &[f32],
    negative_scores: &[f32],
) -> Result<(), io::Error> {
    let mut file = File::create(filename)?;

    // Write CSV header
    writeln!(file, "Time,Positive,Negative")?;

    // write positive and negative scores side by side
    let max_len = positive_scores.len().max(negative_scores.len());
    for i in 0..max_len {
        let pos = positive_scores.get(i).map(|v| v.to_string()).unwrap_or_default();
        let neg = negative_scores.get(i).map(|v| v.to_string()).unwrap_or_default();
        writeln!(file, "{pos},{neg}")?;
    }

    log::info!("Scores dumped to {filename}");
    Ok(())
}
