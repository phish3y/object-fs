use std::{
    future::Future,
    task::{Context, Poll},
    thread,
    time::Duration,
};

use futures::task::noop_waker_ref;

pub fn poll_until_ready_error<Fut, T, E>(future: Fut) -> Result<T, E>
where
    Fut: Future<Output = Result<T, E>>,
{
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(noop_waker_ref());

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => {
                return result;
            }
            Poll::Pending => {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub fn poll_until_ready<Fut, T>(future: Fut) -> T
where
    Fut: Future<Output = T>,
{
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(noop_waker_ref());

    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => {
                return result;
            }
            Poll::Pending => {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
