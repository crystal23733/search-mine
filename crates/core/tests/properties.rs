use liar_core::board::{BoardSpec, Cell, CellId};
use liar_core::generator::{BoardGenerator, GenerationBudget, verify_certificate};
use liar_core::solver::SolverBudget;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn generated_boards_have_exact_mines_zero_opening_and_replayable_certificates(seed in any::<u64>()) {
        let spec = BoardSpec::default();
        let generated = BoardGenerator::generate(spec, seed, GenerationBudget::default())
            .unwrap_or_else(|e| panic!("generation failed: {e:?}"));
        let mines = (0..256).filter(|&i| generated.board.cell(CellId(i)) == Some(Cell::Mine)).count();
        prop_assert_eq!(mines, 40);
        prop_assert_eq!(generated.board.cell(spec.opening), Some(Cell::Number(0)));
        prop_assert!(verify_certificate(&generated.board, &generated.certificate, SolverBudget::default()));
        prop_assert_eq!(generated.stats.candidates, generated.stats.stalled + generated.stats.budget_rejections + 1);
    }
}
