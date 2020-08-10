#![feature(generators, generator_trait, try_trait)]

use std::future::Future;

#[propane::generator]
async fn foo<F: Future>(fut: F) -> i32 {
    fut.await;
    yield 0i32;
}

#[propane::generator]
async fn stream<T, F: Future<Output = T>>(futures: Vec<F>) -> T {
    for future in futures {
        yield future.await;
    }
}
