use bevy::math::Vec2;
use shine_game::math::{unistroke_templates, GestureId, JackknifeClassifier, JackknifeConfig, JackknifeTemplateSet};
use shine_test::test;
use std::fs;
use std::path::{Path, PathBuf};

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
fn test_jackknife_foo() {
    let config = JackknifeConfig::euclidean_distance();

    let mut template_set = JackknifeTemplateSet::new(config.clone());

    template_set.add_template(GestureId(0), unistroke_templates::ZIG_ZAG);
    template_set.add_template(
        GestureId(0),
        &[
            Vec2::new(207.0, 16.0),
            Vec2::new(313.0, 86.0),
            Vec2::new(356.0, 315.0),
            Vec2::new(375.0, 386.0),
            Vec2::new(399.0, 416.0),
            Vec2::new(418.0, 486.0),
        ],
    );
    let sample = &[
        Vec2::new(247.0, 164.0),
        Vec2::new(313.0, 862.0),
        Vec2::new(336.0, 115.0),
        Vec2::new(325.0, 386.0),
        Vec2::new(399.0, 456.0),
        Vec2::new(118.0, 476.0),
    ];

    //template_set.train_rejection();

    let mut classify = JackknifeClassifier::new();
    classify.classify(&template_set, sample);

    /*log::info!("template: {}", serde_json::to_string_pretty(&template_set).unwrap());
    log::info!(
        "sample: {}",
        serde_json::to_string_pretty(&classify.sample_features().unwrap()).unwrap()
    );
    log::info!("correction_factors: {:?}", classify.internal().correction_factors);
    log::info!("lower_bounds: {:?}", classify.internal().lower_bounds);
    log::info!("classification: {:?}", classify.classification());*/
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
