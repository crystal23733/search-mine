use super::*;
use std::sync::{Condvar, Mutex, mpsc};

struct Gate {
    open: Mutex<bool>,
    wake: Condvar,
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}
impl Gate {
    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
impl SeedSource for Gate {
    fn seed(&self) -> Result<[u8; 8], liar_protocol::online::OnlineError> {
        if let Some(entered) = self.entered.lock().unwrap().take() {
            let _ = entered.send(());
        }
        let mut open = self.open.lock().unwrap();
        while !*open {
            open = self.wake.wait(open).unwrap();
        }
        Ok(42_u64.to_le_bytes())
    }
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[test]
fn fatal_result_returns_before_actual_board_worker_is_released() {
    let (entered, observed) = tokio::sync::oneshot::channel();
    let gate = Arc::new(Gate {
        open: Mutex::new(false),
        wake: Condvar::new(),
        entered: Mutex::new(Some(entered)),
    });
    let _release = Release(gate.clone());
    let (ready, ready_result) = mpsc::channel();
    let (returned, result) = mpsc::channel();
    let worker = {
        let gate = gate.clone();
        std::thread::spawn(move || {
            let failed = run(async move {
                let pool = BoardPool::start(liar_core::rules::RulesSnapshot::bundled(), 1, 1, gate)
                    .unwrap();
                observed.await.unwrap();
                ready.send(()).unwrap();
                drop(pool);
                Err("Online storage ownership lost".into())
            })
            .is_err();
            returned.send(failed).unwrap();
        })
    };
    ready_result.recv_timeout(Duration::from_secs(4)).unwrap();
    let returned_before_release = result.recv_timeout(Duration::from_secs(2));
    gate.release();
    worker.join().unwrap();
    assert_eq!(returned_before_release, Ok(true));
}

#[test]
fn normal_server_success_and_fatal_error_are_preserved() {
    assert!(run(async { Ok(()) }).is_ok());
    let error = run(async { Err("Server bind failed".into()) }).unwrap_err();
    assert_eq!(error.to_string(), "Server bind failed");
}
