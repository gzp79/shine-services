use core::f32;
use shine_game::math::statistics::{RunningMoments, ScoringMode};
use shine_game::math::{statistics, TrainLegend};
use shine_game::math::{unistroke_templates, GestureId, JackknifeClassifier, JackknifeConfig, JackknifeTemplateSet};
use shine_test::test;
use std::{
    fs,
    path::{Path, PathBuf},
};

type KinectPoint = [f32; 63];

fn load_kinect_sample(path: &Path) -> Vec<KinectPoint> {
    log::info!("Loading Kinect sample from: {path:?}");
    let content = fs::read_to_string(path).unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut kinect_points = Vec::new();

    // Skip header lines until first "####"
    let mut i = 0;
    while i < lines.len() && lines[i] != "####" {
        i += 1;
    }
    i += 1; // Skip the "####" line

    // Parse frames
    while i < lines.len() {
        if lines[i] == "####" {
            i += 1; // Skip to next frame
            continue;
        }

        // Parse 21 lines of coordinate data for one frame
        let mut point = [0.0f32; 63];
        let mut coord_index = 0;

        for _ in 0..21 {
            if i >= lines.len() || lines[i] == "####" {
                break;
            }

            let coords: Vec<&str> = lines[i].split(',').collect();
            if coords.len() == 3 {
                for coord_str in coords {
                    if let Ok(coord) = coord_str.parse::<f32>() {
                        if coord_index < 63 {
                            point[coord_index] = coord;
                            coord_index += 1;
                        }
                    }
                }
            }
            i += 1;
        }

        // Only add the point if we have all 63 coordinates
        if coord_index == 63 {
            kinect_points.push(point);
        }
    }

    kinect_points
}

fn load_kinect_templates(config: &JackknifeConfig, paths: &[(String, GestureId)]) -> JackknifeTemplateSet<KinectPoint> {
    let mut template_set = JackknifeTemplateSet::new(config.clone());

    for (path, gesture_id) in paths {
        let sample = load_kinect_sample(&PathBuf::from(path.as_str()));
        template_set.add_template(*gesture_id, &sample);
    }

    template_set
}

#[test]
fn test_jackknife_train() {
    let config = JackknifeConfig::inner_product();

    /*let mut template_set = {
        let mut template_set = JackknifeTemplateSet::new(config.clone());
        template_set.add_template(GestureId(0), unistroke_templates::LINE_0);
        template_set.add_template(GestureId(1), unistroke_templates::LINE_90);
        template_set.add_template(GestureId(2), unistroke_templates::LINE_180);
        template_set.add_template(GestureId(3), unistroke_templates::LINE_270);
        template_set
    };*/

    /*let mut template_set = {
        let mut template_set = JackknifeTemplateSet::new(config.clone());
        template_set.add_template(GestureId(0), unistroke_templates::CIRCLE);
        template_set.add_template(GestureId(1), unistroke_templates::TRIANGLE);
        template_set.add_template(GestureId(2), unistroke_templates::RECTANGLE);
        template_set
    };*/

    let mut template_set = {
        let mut template_set = JackknifeTemplateSet::new(config.clone());
        template_set.add_template(GestureId(0), unistroke_templates::LINE_0);
        template_set.add_template(GestureId(1), unistroke_templates::LINE_45);
        template_set.add_template(GestureId(2), unistroke_templates::LINE_90);
        template_set.add_template(GestureId(3), unistroke_templates::LINE_135);
        template_set.add_template(GestureId(4), unistroke_templates::LINE_180);
        template_set.add_template(GestureId(5), unistroke_templates::LINE_225);
        template_set.add_template(GestureId(6), unistroke_templates::LINE_270);
        template_set.add_template(GestureId(7), unistroke_templates::LINE_315);
        template_set.add_template(GestureId(8), unistroke_templates::V);
        template_set.add_template(GestureId(9), unistroke_templates::TRIANGLE);
        template_set.add_template(GestureId(10), unistroke_templates::RECTANGLE);
        template_set.add_template(GestureId(11), unistroke_templates::CIRCLE);
        template_set.add_template(GestureId(12), unistroke_templates::ZIG_ZAG);
        template_set
    };

    /*let mut template_set = {
        let trains = {
            let base = "tests/jackknife_data/kinect/train";
            &[
                (format!("{base}/cartwheel_left/ex_1.txt"), GestureId(0)),
                (format!("{base}/cartwheel_left/ex_2.txt"), GestureId(0)),
                (format!("{base}/cartwheel_right/ex_1.txt"), GestureId(2)),
            ]
        };
        load_kinect_templates(&config, trains)
    };*/

    let mut legend = TrainLegend::default();
    template_set.train(
        1000,
        config.resample_count,
        0.25,
        config.resample_count / 5,
        0.2,
        Some(&mut legend),
    );

    for i in 0..legend.len() {
        let mut neg = Vec::new();
        let mut pos = Vec::new();
        let mut moments = Vec::new();

        for (j, samples) in legend[i].iter().enumerate() {
            log::info!("Template {i}, Sample {j}: Positive: {}", samples.is_positive);
            moments.push(RunningMoments::new());
            moments[j].add_slice(&samples.corrected_scores);
            if samples.is_positive {
                pos.extend_from_slice(&samples.corrected_scores);
            } else {
                neg.extend_from_slice(&samples.corrected_scores);
            }
        }

        // dump scores
        {
            let filename = format!("../../temp/train_template_{i}.csv");
            let mut file = fs::File::create(filename).unwrap();
            legend
                .dump(&mut file, i, |i, p, c| {
                    format!(
                        "{:?}_{}_{}",
                        template_set.templates()[i].id().0,
                        if p { "positive" } else { "negative" },
                        if c { "corrected" } else { "score" }
                    )
                })
                .unwrap();
        }

        let roc = statistics::roc(&pos, &neg, ScoringMode::LowerIsBetter);
        //dump roc curve
        {
            let filename = format!("../../temp/roc_{i}.csv");
            let mut file = fs::File::create(filename).unwrap();
            statistics::dump_roc(&roc, &mut file).unwrap();
        }
        let auc = statistics::auc(&roc);
        log::info!("Template {i} AUC: {auc}");

        let th = template_set.templates()[i].rejection_threshold();

        // reclassify scores based on the threshold
        let mut final_neg = Vec::new();
        let mut final_pos = Vec::new();

        for p in pos {
            if p < th {
                final_pos.push(p);
            } else {
                final_neg.push(p);
            }
        }
        for n in neg {
            if n < th {
                final_pos.push(n);
            } else {
                final_neg.push(n);
            }
        }

        let true_roc = statistics::roc(&final_pos, &final_neg, ScoringMode::LowerIsBetter);
        //dump roc curve
        {
            let filename = format!("../../temp/true_roc_{i}.csv");
            let mut file = fs::File::create(filename).unwrap();
            statistics::dump_roc(&true_roc, &mut file).unwrap();
        }
        let true_auc = statistics::auc(&true_roc);
        log::info!("Template {i} Threshold: {th} AUC: {auc} -> {true_auc}");
    }
}

#[test]
fn test_jackknife_kinect_classification() {
    log::info!("Working directory: {:?}", std::env::current_dir().unwrap());
    let trains = {
        let base = "tests/jackknife_data/kinect/train";
        &[
            (format!("{base}/cartwheel_left/ex_1.txt"), GestureId(0)),
            (format!("{base}/cartwheel_left/ex_2.txt"), GestureId(0)),
            (format!("{base}/cartwheel_right/ex_1.txt"), GestureId(1)),
            (format!("{base}/cartwheel_right/ex_2.txt"), GestureId(1)),
            (format!("{base}/duck/ex_1.txt"), GestureId(2)),
            (format!("{base}/duck/ex_2.txt"), GestureId(2)),
            (format!("{base}/hook_left/ex_1.txt"), GestureId(3)),
            (format!("{base}/hook_left/ex_2.txt"), GestureId(3)),
            (format!("{base}/hook_right/ex_1.txt"), GestureId(4)),
            (format!("{base}/hook_right/ex_2.txt"), GestureId(4)),
            (format!("{base}/jab_left/ex_1.txt"), GestureId(5)),
            (format!("{base}/jab_left/ex_2.txt"), GestureId(5)),
            (format!("{base}/jab_right/ex_1.txt"), GestureId(6)),
            (format!("{base}/jab_right/ex_2.txt"), GestureId(6)),
            (format!("{base}/kick_left/ex_1.txt"), GestureId(7)),
            (format!("{base}/kick_left/ex_2.txt"), GestureId(7)),
            (format!("{base}/kick_right/ex_1.txt"), GestureId(8)),
            (format!("{base}/kick_right/ex_2.txt"), GestureId(8)),
            (format!("{base}/push/ex_1.txt"), GestureId(9)),
            (format!("{base}/push/ex_2.txt"), GestureId(9)),
        ]
    };
    let samples = {
        let base = "tests/jackknife_data/kinect/samples";
        [
            (format!("{base}/cartwheel_left/ex_01.txt"), Some(GestureId(0))),
            (format!("{base}/cartwheel_left/ex_02.txt"), Some(GestureId(0))),
            (format!("{base}/cartwheel_left/ex_11.txt"), Some(GestureId(0))),
            (format!("{base}/cartwheel_left/ex_12.txt"), Some(GestureId(0))),
            (format!("{base}/jab_right/ex_01.txt"), Some(GestureId(6))),
            (format!("{base}/jab_right/ex_02.txt"), Some(GestureId(6))),
            (format!("{base}/jab_right/ex_11.txt"), Some(GestureId(6))),
            (format!("{base}/jab_right/ex_12.txt"), Some(GestureId(6))),
            // require lower bound and training to have fail ("tests/jackknife_data/kinect/samples/uppercut_right/ex_01.txt", None), // shall fail as not in gesture set
        ]
    };

    let config = JackknifeConfig::inner_product();
    let template_set = load_kinect_templates(&config, trains);
    let mut classifier = JackknifeClassifier::new();

    for (sample, expected) in samples {
        let points = load_kinect_sample(&PathBuf::from(&sample));
        let result = classifier.classify(&template_set, &points);

        if let Some((index, cost)) = result {
            let (name, gesture_id) = &trains[index];
            log::info!("Sample: {sample} ({expected:?}), Classified as: {name} ({index}), Cost: {cost}");
            assert_eq!(Some(*gesture_id), expected);
        } else {
            log::warn!("Sample: {sample} could not be classified");
            assert!(expected.is_none());
        }
    }
}
