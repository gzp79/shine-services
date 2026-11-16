use tokio::task::JoinHandle;

pub async fn join<T>(handles: Vec<JoinHandle<T>>) -> Vec<T> {
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }
    results
}
