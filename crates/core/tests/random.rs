use liar_core::random::{RandomSource, shuffle};
struct ScriptedRandom(std::vec::IntoIter<u64>);
impl RandomSource for ScriptedRandom {
    fn next_u64(&mut self) -> u64 {
        self.0
            .next()
            .expect("only the scripted draws may be consumed")
    }
}
#[test]
fn shuffle_rejects_biased_draws_before_selecting_a_bounded_index() {
    let mut random = ScriptedRandom(vec![0, 5, 0].into_iter());
    let mut cells = [0, 1, 2];
    shuffle(&mut cells, &mut random);
    assert_eq!(cells, [1, 0, 2]);
    assert_eq!(random.0.next(), None);
}
