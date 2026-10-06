use liar_core::board::{BoardSpec, Cell, CellId, Observation, ObservedCell};
use liar_core::generator::{BoardGenerator, GenerationBudget};
use liar_core::knowledge::{KnowledgeMemory, KnowledgeSolver};
use liar_core::lie::{LieRejected, LieValidator};
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: u64 = env::args()
        .nth(1)
        .ok_or("Usage: lie_bench SEEDS CSV_PATH")?
        .parse()?;
    if count == 0 {
        return Err("SEEDS must be positive".into());
    }
    let mut csv = BufWriter::new(File::create(
        env::args().nth(2).ok_or("CSV_PATH required")?,
    )?);
    writeln!(
        csv,
        "seed,outcome,steps,states,positive_candidates,certificates,microseconds"
    )?;
    let spec = BoardSpec::default();
    let flags = vec![false; spec.area()];
    let mut complete = 0u64;
    let mut stalled = 0u64;
    let mut budget = 0u64;
    let mut states = 0u64;
    let mut candidates = 0u64;
    let mut certificates = 0u64;
    let mut times = Vec::new();
    for seed in 0..count {
        let generated = BoardGenerator::generate(spec, seed, GenerationBudget::default())
            .map_err(|e| format!("generation failed: {e:?}"))?;
        let board = generated.board;
        let mut view =
            Observation::closed(spec).map_err(|_| "observation initialization failed")?;
        board
            .reveal(&mut view, spec.opening, &flags)
            .map_err(|_| "opening failed")?;
        let mut memory = KnowledgeMemory::from_initial_opening(&view)
            .map_err(|_| "opening provenance failed")?;
        let mut steps = 0;
        let mut seed_states = 0;
        let mut seed_candidates = 0;
        let mut seed_certificates = 0;
        let tick = Instant::now();
        let outcome = loop {
            if view.opened_safe() == board.safe_total() {
                complete += 1;
                break "complete";
            }
            let started = Instant::now();
            let prepared = LieValidator::prepare(&view, &memory, KnowledgeSolver::default_budget());
            times.push(started.elapsed().as_micros() as u64);
            seed_states += 1;
            states += 1;
            let prepared = match prepared {
                Ok(proof) => proof,
                Err(LieRejected::BudgetExceeded) => {
                    budget += 1;
                    break "budget";
                }
                Err(error) => {
                    return Err(
                        format!("Unexpected preparation error for seed {seed}: {error:?}").into(),
                    );
                }
            };
            for i in 0..spec.area() {
                let cell = CellId(i as u16);
                if view.cell(cell) != Some(ObservedCell::Unknown) {
                    continue;
                }
                let Some(Cell::Number(truth @ 1..=8)) = board.cell(cell) else {
                    continue;
                };
                for delta in [-1i8, 1] {
                    if !(1..=8).contains(&(i16::from(truth) + i16::from(delta))) {
                        continue;
                    }
                    seed_candidates += 1;
                    candidates += 1;
                    if let Ok(certificate) = prepared.certificate(cell, delta, 0) {
                        if certificate.expected_truth() != truth {
                            return Err("Unsound public proof".into());
                        }
                        seed_certificates += 1;
                        certificates += 1;
                    }
                }
            }
            let safe = prepared.inference().safe().to_vec();
            if safe.is_empty() {
                stalled += 1;
                break "stalled";
            }
            prepared
                .remember(&mut memory)
                .map_err(|_| "proof memory conflict")?;
            for cell in safe {
                if !matches!(board.cell(cell), Some(Cell::Number(_))) {
                    return Err("Unsound safe inference".into());
                }
                board
                    .reveal(&mut view, cell, &flags)
                    .map_err(|_| "reveal failed")?;
            }
            steps += 1;
        };
        writeln!(
            csv,
            "{seed},{outcome},{steps},{seed_states},{seed_candidates},{seed_certificates},{}",
            tick.elapsed().as_micros()
        )?;
        if (seed + 1) % 100 == 0 {
            eprintln!("Assessed {}/{}", seed + 1, count);
        }
    }
    csv.flush()?;
    times.sort_unstable();
    let percentile = |percent: usize| {
        times
            .get(
                ((times.len() * percent).div_ceil(100).saturating_sub(1))
                    .min(times.len().saturating_sub(1)),
            )
            .copied()
            .unwrap_or(0)
    };
    println!(
        "{{\"seeds\":{count},\"complete\":{complete},\"stalled\":{stalled},\"budget\":{budget},\"states\":{states},\"positive_candidates\":{candidates},\"certificates\":{certificates},\"p50_us\":{},\"p95_us\":{},\"max_us\":{}}}",
        percentile(50),
        percentile(95),
        times.last().copied().unwrap_or(0)
    );
    Ok(())
}
