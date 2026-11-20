// see: cans/03-sdk-architecture.md#Full-surface-examples
pub use tokio::fs;
pub use tokio::task::JoinSet;
pub use tokio::time;

// see: cans/03-sdk-architecture.md#Full-surface-examples
pub fn block_on<F: Future<Output = T>, T>(future: F) -> std::io::Result<T> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(future))
}
