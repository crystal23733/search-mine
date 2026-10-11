use super::*;
use std::sync::{Condvar, Mutex};
fn blocked(
    workers: ExportWorkers,
    entered: tokio::sync::mpsc::UnboundedSender<()>,
    gate: Arc<(Mutex<bool>, Condvar)>,
) -> tokio::task::JoinHandle<Result<(), ExportError>> {
    tokio::spawn(async move {
        workers
            .run(move || {
                entered.send(()).unwrap();
                let (lock, wake) = &*gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = wake.wait(open).unwrap();
                }
                Ok(())
            })
            .await
    })
}
#[tokio::test]
async fn caller_cancellation_and_timeout_do_not_release_a_running_worker() {
    let workers = ExportWorkers::default();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let a = blocked(workers.clone(), tx.clone(), gate.clone());
    let b = blocked(workers.clone(), tx, gate.clone());
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
    }
    a.abort();
    assert_eq!(workers.run(|| Ok(())).await, Err(ExportError::Unavailable));
    assert_eq!(b.await.unwrap(), Err(ExportError::Unavailable));
    assert_eq!(workers.run(|| Ok(())).await, Err(ExportError::Unavailable));
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if workers.run(|| Ok(())).await.is_ok() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        workers
            .run(|| -> Result<(), ExportError> { panic!("injected worker failure") })
            .await,
        Err(ExportError::Unavailable)
    );
    assert_eq!(workers.run(|| Ok(7)).await, Ok(7));
}
