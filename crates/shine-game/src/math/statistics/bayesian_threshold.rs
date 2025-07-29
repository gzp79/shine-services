/// Calculate the Bayesian threshold between two distributions.
pub fn bayesian_threshold(mean1: f32, std1: f32, mean2: f32, std2: f32) -> f32 {
    let var1 = std1 * std1;
    let var2 = std2 * std2;

    if (var1 - var2).abs() < f32::EPSILON {
        // Equal variances
        (mean1 + mean2) / 2.0 + (var1 / (mean2 - mean1)).ln()
    } else {
        // Unequal variances - solve quadratic equation
        let a = var2 - var1;
        let b = 2.0 * (mean1 * var2 - mean2 * var1);
        let c = var1 * mean2.powi(2) - var2 * mean1.powi(2) + 2.0 * var1 * var2.ln();

        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let sqrt_disc = discriminant.sqrt();
            let t1 = (-b + sqrt_disc) / (2.0 * a);
            let t2 = (-b - sqrt_disc) / (2.0 * a);

            let min_mean = mean1.min(mean2);
            let max_mean = mean1.max(mean2);

            // Choose the threshold between the means
            let t1_valid = t1 >= min_mean && t1 <= max_mean;
            let t2_valid = t2 >= min_mean && t2 <= max_mean;

            if t1_valid && t2_valid {
                // Both solutions are valid, choose the one closer to the midpoint
                let midpoint = (mean1 + mean2) / 2.0;
                if (t1 - midpoint).abs() < (t2 - midpoint).abs() {
                    t1
                } else {
                    t2
                }
            } else if t1_valid {
                t1
            } else if t2_valid {
                t2
            } else {
                // Neither solution is between the means, use fallback
                (mean1 + mean2) / 2.0
            }
        } else {
            (mean1 + mean2) / 2.0 // Fallback
        }
    }
}
