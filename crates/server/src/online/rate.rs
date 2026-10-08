pub struct CommandRate {
    last: u64,
    credits: u64,
}
impl CommandRate {
    pub fn new(now: u64) -> Self {
        Self {
            last: now,
            credits: 40000,
        }
    }
    pub fn permit(&mut self, now: u64) -> bool {
        if now < self.last {
            return false;
        }
        self.credits = self
            .credits
            .saturating_add(now.saturating_sub(self.last).saturating_mul(20))
            .min(40000);
        self.last = now;
        if self.credits < 1000 {
            return false;
        }
        self.credits -= 1000;
        true
    }
}
