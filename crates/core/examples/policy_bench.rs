//! Legal public-strategy corpus; Board is used only to execute and independently check proofs.
use liar_core::board::{BoardSpec, Cell, CellId, Observation, ObservedCell};
use liar_core::generator::{BoardGenerator, GenerationBudget};
use liar_core::policy::{ATTACK_STRATEGY, PolicyKnowledge};
use liar_core::solver::SolverBudget;
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: u64 = env::args()
        .nth(1)
        .ok_or("Usage: policy_bench SEEDS CSV_PATH")?
        .parse()?;
    if count == 0 {
        return Err("SEEDS must be positive".into());
    }
    let mut csv = BufWriter::new(File::create(
        env::args().nth(2).ok_or("CSV_PATH required")?,
    )?);
    writeln!(
        csv,
        "seed,outcome,steps,positive_candidates,certificates,applied_lies,accusations,microseconds"
    )?;
    let spec = BoardSpec::default();
    let flags = vec![false; spec.area()];
    let (mut completed, mut offered, mut accepted, mut attacked, mut accused, mut double) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut times = Vec::new();
    for seed in 0..count {
        let generated = BoardGenerator::generate(spec, seed, GenerationBudget::default())
            .map_err(|e| format!("generation: {e:?}"))?;
        let board = generated.board;
        let mut view = Observation::closed(spec).map_err(|_| "invalid spec")?;
        board
            .reveal(&mut view, spec.opening, &flags)
            .map_err(|_| "opening failed")?;
        let initial = view.clone();
        let tick = Instant::now();
        let mut ledger = PolicyKnowledge::start(&initial, SolverBudget::default())
            .map_err(|e| format!("start: {e:?}"))?;
        let mut overlays: Vec<(CellId, u8, u8)> = Vec::new();
        let (mut steps, mut candidates, mut certificates, mut attacks, mut accusations) =
            (0u64, 0u64, 0u64, 0u64, 0u64);
        while view.opened_safe() < board.safe_total() {
            // Unknown active count and location are never passed into the public inference.
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
                    candidates += 1;
                    if let Ok(proof) = ledger.certificate(cell, delta, 0) {
                        if proof.expected_truth() != truth || !proof.matches(&ledger) {
                            return Err("unsound certificate".into());
                        }
                        certificates += 1;
                        if overlays.len() < 2 && !overlays.iter().any(|p| p.0 == cell) {
                            overlays.push((cell, proof.expected_truth(), proof.displayed()));
                            attacks += 1;
                        }
                    }
                }
            }
            if overlays.len() == 2 {
                double += 1;
            }
            let safe = ledger.safe();
            if safe.is_empty() {
                return Err(format!("seed {seed} stalled: {:?}", ledger.analysis()).into());
            }
            for cell in safe {
                if !matches!(board.cell(cell), Some(Cell::Number(_))) {
                    return Err("unsafe public inference".into());
                }
                board
                    .reveal(&mut view, cell, &flags)
                    .map_err(|_| "reveal failed")?;
            }
            for &(cell, _, shown) in &overlays {
                if matches!(view.cell(cell), Some(ObservedCell::Number(_))) {
                    view.set(cell, ObservedCell::Number(shown))
                        .map_err(|_| "overlay failed")?;
                }
            }
            let started = Instant::now();
            ledger
                .observe(&view)
                .map_err(|e| format!("observe: {e:?}"))?;
            ledger.refresh().map_err(|e| format!("refresh: {e:?}"))?;
            times.push(started.elapsed().as_micros() as u64);
            for cell in ledger.known_lies() {
                let index = overlays
                    .iter()
                    .position(|p| p.0 == cell)
                    .ok_or("false accusation")?;
                let (_, truth, _) = overlays.remove(index);
                view.set(cell, ObservedCell::Number(truth))
                    .map_err(|_| "accusation failed")?;
                accusations += 1;
            }
            ledger
                .observe(&view)
                .map_err(|e| format!("correction: {e:?}"))?;
            ledger
                .refresh()
                .map_err(|e| format!("post correction: {e:?}"))?;
            steps += 1;
            if steps > spec.area() as u64 {
                return Err("non-terminating proof path".into());
            }
        }
        let restored =
            PolicyKnowledge::replay(&initial, ledger.public_history(), SolverBudget::default())
                .map_err(|e| format!("replay: {e:?}"))?;
        if ledger != restored {
            return Err("public history differs on replay".into());
        }
        completed += 1;
        offered += candidates;
        accepted += certificates;
        attacked += attacks;
        accused += accusations;
        writeln!(
            csv,
            "{seed},complete,{steps},{candidates},{certificates},{attacks},{accusations},{}",
            tick.elapsed().as_micros()
        )?;
        if (seed + 1) % 100 == 0 {
            eprintln!("Assessed {}/{}", seed + 1, count);
        }
    }
    csv.flush()?;
    times.sort_unstable();
    let percentile = |p: usize| {
        times
            .get((times.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
            .unwrap_or(0)
    };
    println!(
        "{{\"strategy\":\"{ATTACK_STRATEGY}\",\"seeds\":{count},\"complete\":{completed},\"positive_candidates\":{offered},\"certificates\":{accepted},\"applied_lies\":{attacked},\"accusations\":{accused},\"two_active_states\":{double},\"p50_us\":{},\"p95_us\":{},\"max_us\":{}}}",
        percentile(50),
        percentile(95),
        times.last().copied().unwrap_or(0)
    );
    Ok(())
}
