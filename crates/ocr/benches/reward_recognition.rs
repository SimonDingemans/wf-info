use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use ocr::{
    FeatureScanner, OcrOptions,
    debug::reward_screen_fixture_frame,
    implementations::reward_screen::{
        RewardScreenScanner, RewardUiTheme, clear_cached_reward_screen_scanner,
        scan_reward_screen_frame,
    },
    tesseract::TesseractRecognizer,
};

const TARGET_LATENCY: Duration = Duration::from_millis(1500);
const COLD_ITERATIONS: usize = 3;
const WARM_ITERATIONS: usize = 10;

fn main() {
    let frame = reward_screen_fixture_frame().expect("reward fixture should load");
    let options = OcrOptions::default();

    let cold = benchmark_cold_scans(&frame, &options);
    let warm = benchmark_warm_cached_scans(&frame, &options);

    print_report("cold scanner build + OCR", &cold);
    print_report("warm cached OCR", &warm);
    print_budget("warm cached OCR", warm.mean());

    clear_cached_reward_screen_scanner().expect("cached scanner should clear");
}

fn benchmark_cold_scans(frame: &ocr::CapturedFrame, options: &OcrOptions) -> BenchRun {
    let mut run = BenchRun::new(COLD_ITERATIONS);

    for _ in 0..COLD_ITERATIONS {
        let started = Instant::now();
        let recognizer = TesseractRecognizer::new(options).expect("Tesseract should initialize");
        let scanner =
            RewardScreenScanner::new(recognizer, RewardUiTheme::Vitruvian, options.clone());
        let rewards = scanner
            .scan(black_box(frame))
            .expect("reward OCR should run");
        black_box(rewards);
        run.push(started.elapsed());
    }

    run
}

fn benchmark_warm_cached_scans(frame: &ocr::CapturedFrame, options: &OcrOptions) -> BenchRun {
    let _ = scan_reward_screen_frame(frame, RewardUiTheme::Vitruvian, options.clone())
        .expect("warmup reward OCR should run");

    let mut run = BenchRun::new(WARM_ITERATIONS);

    for _ in 0..WARM_ITERATIONS {
        let started = Instant::now();
        let rewards =
            scan_reward_screen_frame(black_box(frame), RewardUiTheme::Vitruvian, options.clone())
                .expect("cached reward OCR should run");
        black_box(rewards);
        run.push(started.elapsed());
    }

    run
}

fn print_report(label: &str, run: &BenchRun) {
    println!(
        "{label}: iterations={} mean={} min={} max={} p95={}",
        run.len(),
        format_duration(run.mean()),
        format_duration(run.min()),
        format_duration(run.max()),
        format_duration(run.p95())
    );
}

fn print_budget(label: &str, duration: Duration) {
    let status = if duration <= TARGET_LATENCY {
        "within"
    } else {
        "over"
    };

    println!(
        "{label}: mean {} is {status} the {} target",
        format_duration(duration),
        format_duration(TARGET_LATENCY)
    );
}

fn format_duration(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1000.0)
}

#[derive(Debug)]
struct BenchRun {
    samples: Vec<Duration>,
}

impl BenchRun {
    fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, sample: Duration) {
        self.samples.push(sample);
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn mean(&self) -> Duration {
        let total = self.samples.iter().sum::<Duration>();
        total / self.samples.len() as u32
    }

    fn min(&self) -> Duration {
        *self
            .samples
            .iter()
            .min()
            .expect("benchmark should have at least one sample")
    }

    fn max(&self) -> Duration {
        *self
            .samples
            .iter()
            .max()
            .expect("benchmark should have at least one sample")
    }

    fn p95(&self) -> Duration {
        let mut samples = self.samples.clone();
        samples.sort_unstable();
        let index = samples.len().saturating_sub(1) * 95 / 100;

        samples[index]
    }
}
