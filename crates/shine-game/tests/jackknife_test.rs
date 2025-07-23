use shine_game::ai::{JackknifeClassifier, JackknifeConfig, JackknifeTemplateSet};
use shine_test::test;
use std::fs;
use std::path::{Path, PathBuf};

type KinectPoint = [f32; 63];

fn load_kinect_sample(path: &Path) -> Vec<KinectPoint> {
    log::info!("Loading Kinect sample from: {:?}", path);
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

fn load_kinect_templates(config: &JackknifeConfig, paths: &[&str]) -> JackknifeTemplateSet<KinectPoint> {
    let mut template_set = JackknifeTemplateSet::new(config.clone());

    for &path in paths {
        let sample = load_kinect_sample(&PathBuf::from(path));
        template_set.add_template_from_points(&sample);
    }

    template_set
}

#[test]
fn test_jackknife_feature() {
    log::info!("Working directory: {:?}", std::env::current_dir().unwrap());

    let config = JackknifeConfig::inner_product();
    let mut classifier = JackknifeClassifier::new(config.clone());

    let templates = &[
        "tests/jackknife_data/kinect/train/cartwheel_left.txt",
        "tests/jackknife_data/kinect/train/cartwheel_right.txt",
        "tests/jackknife_data/kinect/train/duck.txt",
        "tests/jackknife_data/kinect/train/hook_left.txt",
        "tests/jackknife_data/kinect/train/hook_right.txt",
        "tests/jackknife_data/kinect/train/jab_left.txt",
        "tests/jackknife_data/kinect/train/jab_right.txt",
        "tests/jackknife_data/kinect/train/kick_left.txt",
        "tests/jackknife_data/kinect/train/kick_right.txt",
        "tests/jackknife_data/kinect/train/push.txt",
    ];
    let template_set = load_kinect_templates(&config, templates);

    let samples = [
        ("tests/jackknife_data/kinect/samples/cartwheel_left/ex_01.txt", Some(0)),
        ("tests/jackknife_data/kinect/samples/cartwheel_left/ex_02.txt", Some(0)),
        ("tests/jackknife_data/kinect/samples/cartwheel_left/ex_11.txt", Some(0)),
        ("tests/jackknife_data/kinect/samples/cartwheel_left/ex_12.txt", Some(0)),
        ("tests/jackknife_data/kinect/samples/jab_right/ex_01.txt", Some(6)),
        ("tests/jackknife_data/kinect/samples/jab_right/ex_02.txt", Some(6)),
        ("tests/jackknife_data/kinect/samples/jab_right/ex_11.txt", Some(6)),
        ("tests/jackknife_data/kinect/samples/jab_right/ex_12.txt", Some(6)),
        // ("tests/jackknife_data/kinect/samples/uppercut_right/ex_01.txt", None), // not in the templates - require lower bound to fail
    ];
    for (sample, expected) in samples {
        let points = load_kinect_sample(&PathBuf::from(sample));
        let result = classifier.classify(&points, &template_set.templates());

        if let Some((index, cost)) = result {
            let name = templates[index];
            let expected = expected.map(|e| e.to_string()).unwrap_or("not classified".to_string());
            log::info!("Sample: {sample} ({expected}), Classified as: {name} ({index}), Cost: {cost}");
        } else {
            log::warn!("Sample: {} could not be classified", sample);
        }

        assert_eq!(result.map(|(index, _)| index), expected);
    }
}
