// SPDX-License-Identifier: AGPL-3.0-or-later

#[derive(Clone, Copy)]
struct Rect {
    system: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Rect {
    fn contains_center(self, candidate: Self) -> bool {
        let x = candidate.x + candidate.width / 2;
        let y = candidate.y + candidate.height / 2;
        self.system == candidate.system
            && x >= self.x
            && x <= self.x + self.width
            && y >= self.y
            && y <= self.y + self.height
    }
}

fn fixture() -> (Vec<Rect>, Vec<Rect>, Vec<Rect>) {
    let mut truth = Vec::new();
    let mut baseline = Vec::new();
    let mut high_precision = Vec::new();
    for line in include_str!("../../../oracle/chopin-op09-no01-page1-beam-truth.txt").lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        assert_eq!(fields.len(), 6, "invalid truth row: {line}");
        let rect = Rect {
            system: fields[1].parse().unwrap(),
            x: fields[2].parse().unwrap(),
            y: fields[3].parse().unwrap(),
            width: fields[4].parse().unwrap(),
            height: fields[5].parse().unwrap(),
        };
        match fields[0] {
            "truth" => truth.push(rect),
            "baseline" => baseline.push(rect),
            "high_precision" => high_precision.push(rect),
            other => panic!("unknown truth row kind {other}"),
        }
    }
    (truth, baseline, high_precision)
}

fn precision(truth: &[Rect], candidates: &[Rect]) -> f64 {
    candidates
        .iter()
        .filter(|candidate| truth.iter().any(|area| area.contains_center(**candidate)))
        .count() as f64
        / candidates.len() as f64
}

fn corridor_recall(truth: &[Rect], candidates: &[Rect]) -> f64 {
    truth
        .iter()
        .map(|area| {
            (area.x..=area.x + area.width)
                .filter(|x| {
                    candidates.iter().any(|candidate| {
                        candidate.system == area.system
                            && *x >= candidate.x
                            && *x <= candidate.x + candidate.width
                            && candidate.y + candidate.height / 2 >= area.y
                            && candidate.y + candidate.height / 2 <= area.y + area.height
                    })
                })
                .count() as f64
                / f64::from(area.width + 1)
        })
        .sum::<f64>()
        / truth.len() as f64
}

#[test]
fn chopin_page_one_high_precision_mode_improves_musical_beam_quality() {
    let (truth, baseline, high_precision) = fixture();
    let baseline_precision = precision(&truth, &baseline);
    let baseline_recall = corridor_recall(&truth, &baseline);
    let improved_precision = precision(&truth, &high_precision);
    let improved_recall = corridor_recall(&truth, &high_precision);

    assert!(
        baseline_precision < 0.10,
        "baseline precision {baseline_precision}"
    );
    assert!(baseline_recall < 0.10, "baseline recall {baseline_recall}");
    assert!(
        improved_precision >= 0.90,
        "improved precision {improved_precision}"
    );
    assert!(improved_recall >= 0.40, "improved recall {improved_recall}");
    assert!(improved_precision > baseline_precision + 0.75);
    assert!(improved_recall > baseline_recall + 0.30);
}
