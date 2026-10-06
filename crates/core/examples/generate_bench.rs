use liar_core::board::BoardSpec;
use liar_core::generator::{BoardGenerator, GenerationBudget};
use liar_core::random::RNG_VERSION;
use liar_core::solver::SOLVER_VERSION;
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: u64 = env::args()
        .nth(1)
        .ok_or("Usage: generate_bench SEEDS CSV_PATH")?
        .parse()?;
    if count == 0 {
        return Err("SEEDS must be positive".into());
    }
    let mut csv = BufWriter::new(File::create(
        env::args().nth(2).ok_or("CSV_PATH required")?,
    )?);
    writeln!(
        csv,
        "seed,microseconds,success,candidates,stalled,budget_rejections"
    )?;
    let mut samples = Vec::new();
    let mut failures = 0;
    let mut candidates = 0u64;
    let mut stalled = 0u64;
    let mut budget_rejections = 0u64;
    let started = Instant::now();
    for seed in 0..count {
        let tick = Instant::now();
        let generated =
            BoardGenerator::generate(BoardSpec::default(), seed, GenerationBudget::default());
        let elapsed = tick.elapsed().as_micros() as u64;
        let (success, attempts, stuck, rejected) = match generated {
            Ok(generated) => (
                true,
                generated.stats.candidates,
                generated.stats.stalled,
                generated.stats.budget_rejections,
            ),
            Err(liar_core::generator::GenerationError::Exhausted {
                candidates,
                stalled,
                budget_rejections,
            }) => {
                failures += 1;
                (false, candidates, stalled, budget_rejections)
            }
            Err(error) => {
                return Err(
                    format!("Unexpected generation error for seed {seed}: {error:?}").into(),
                );
            }
        };
        samples.push(elapsed);
        candidates += u64::from(attempts);
        stalled += u64::from(stuck);
        budget_rejections += u64::from(rejected);
        writeln!(
            csv,
            "{seed},{elapsed},{success},{attempts},{stuck},{rejected}"
        )?;
        if (seed + 1) % 1000 == 0 {
            eprintln!("Generated {}/{}", seed + 1, count);
        }
    }
    csv.flush()?;
    samples.sort_unstable();
    let percentile = |percent: usize| {
        samples[((samples.len() * percent).div_ceil(100) - 1).min(samples.len() - 1)]
    };
    println!(
        "{{\"seeds\":{count},\"failures\":{failures},\"rng_version\":{RNG_VERSION},\"solver_version\":{SOLVER_VERSION},\"candidates\":{candidates},\"stalled\":{stalled},\"budget_rejections\":{budget_rejections},\"p50_us\":{},\"p95_us\":{},\"max_us\":{},\"total_ms\":{}}}",
        percentile(50),
        percentile(95),
        samples[samples.len() - 1],
        started.elapsed().as_millis()
    );
    if failures > 0 {
        return Err("Some seeds exhausted the generation budget".into());
    }
    Ok(())
}
