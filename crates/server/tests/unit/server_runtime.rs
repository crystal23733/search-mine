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

#[tokio::test(start_paused = true)]
async fn normal_shutdown_waits_for_drain_before_stopping_http() {
    let (entered, observed) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let (http_stopped, mut http_observed) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(serve_until_shutdown(
        async {},
        async move {
            entered.send(()).unwrap();
            released.await.unwrap();
            Ok(())
        },
        stop,
        async move {
            stopped.await.unwrap();
            http_stopped.send(()).unwrap();
            Ok(())
        },
    ));
    observed.await.unwrap();
    assert!(matches!(
        http_observed.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    assert!(!task.is_finished());
    release.send(()).unwrap();
    assert_eq!(task.await.unwrap(), Ok(()));
    assert_eq!(http_observed.await, Ok(()));
}

#[tokio::test(start_paused = true)]
async fn normal_shutdown_has_separate_drain_and_http_deadlines() {
    let start = tokio::time::Instant::now();
    let (stop, _stopped) = tokio::sync::oneshot::channel();
    let result = serve_until_shutdown(
        async {},
        std::future::pending(),
        stop,
        std::future::pending(),
    )
    .await;
    assert_eq!(result, Err("Online drain timed out"));
    assert_eq!(start.elapsed(), Duration::from_secs(300));
    let start = tokio::time::Instant::now();
    let (stop, _stopped) = tokio::sync::oneshot::channel();
    let result =
        serve_until_shutdown(async {}, async { Ok(()) }, stop, std::future::pending()).await;
    assert_eq!(result, Err("HTTP shutdown timed out"));
    assert_eq!(start.elapsed(), Duration::from_secs(5));
}

#[tokio::test]
async fn failed_drain_or_http_never_becomes_a_normal_shutdown() {
    let (stop, _stopped) = tokio::sync::oneshot::channel();
    assert_eq!(
        serve_until_shutdown(
            async {},
            async { Err("Match drain failed") },
            stop,
            std::future::pending()
        )
        .await,
        Err("Match drain failed")
    );
    let (stop, _stopped) = tokio::sync::oneshot::channel();
    assert_eq!(
        serve_until_shutdown(
            std::future::pending(),
            std::future::pending(),
            stop,
            async { Err(std::io::Error::other("fixture")) }
        )
        .await,
        Err("HTTP server failed")
    );
}

#[test]
fn normal_runtime_cleanup_is_bounded_even_if_actual_cpu_work_has_not_finished() {
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
            let normal = run(async move {
                let pool = BoardPool::start(liar_core::rules::RulesSnapshot::bundled(), 1, 1, gate)
                    .unwrap();
                observed.await.unwrap();
                ready.send(()).unwrap();
                drop(pool);
                Ok(())
            })
            .is_ok();
            returned.send(normal).unwrap();
        })
    };
    ready_result.recv_timeout(Duration::from_secs(4)).unwrap();
    let returned_before_release = result.recv_timeout(Duration::from_secs(4));
    gate.release();
    worker.join().unwrap();
    assert_eq!(returned_before_release, Ok(true));
}
