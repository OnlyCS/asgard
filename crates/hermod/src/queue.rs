use async_std::{stream::StreamExt, sync::Arc};
use futures::{
    channel::mpsc::{self, SendError, UnboundedReceiver as MRecv, UnboundedSender as MSend},
    Future, SinkExt,
};

/// # Sender
///
/// A queue that can be used from anywhere. Wrapper for
/// `futures::channel::mpsc::UnboundedSender` and `UnboundedReceiver`. Calling `.emit()` returns
/// an `UnboundedReciever` when `Ok`. It will recieve one event, and then close (unless sending
/// fails).
///
/// ## Example
/// ```no_run
/// use lazy_static::lazy_static;
/// use hermod::Sender;
/// use std::sync::Arc;
/// use std::error::Error;
///
/// lazy_static! {
///     static ref QUEUE: Arc<Sender<String>> = Arc::new(Sender::new(|event, data| Box::pin(async move {
/// 		listener(event).await
/// 	}), 0u32));
/// }
///
/// async fn listener(event: String) -> bool {
/// 	assert_eq!(event, "Hello, world!");
/// 	Ok(true)
/// }
///
/// pub fn get_instance() -> Arc<Sender<String>> {
///     Arc::clone(&QUEUE)
/// }
///
///
/// let recv: mspc::UnboundedReciever<bool> = get_instance().emit("Hello, world!").await; // emit takes impl Into<T> as argument
/// let mut res = recv.next().await.unwrap();
///
/// assert_eq!(res, true);
/// ```
pub struct Sender<T, R>
where
    T: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    sender: MSend<(T, MSend<R>)>,
}

impl<T, R> Sender<T, R>
where
    T: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    pub fn new<D: Send + Sync + 'static, F>(listener: fn(T, &mut D) -> F, data: D) -> Self
    where
        F: Future<Output = R> + Send + Sync + 'static,
    {
        let (sender, mut receiver) = mpsc::unbounded::<(T, MSend<R>)>();

        async_std::task::spawn(async move {
            let mut data = data;

            while let Some((event, mut sender)) = receiver.next().await {
                let res = listener(event, &mut data).await;

                if let Err(e) = sender.send(res).await {
                    eprintln!("Error sending response: {:?}", e);
                }
            }
        });

        Sender { sender }
    }

    pub async fn emit(self: Arc<Self>, event: impl Into<T>) -> Result<MRecv<R>, SendError> {
        let (sender, receiver) = mpsc::unbounded();
        self.sender.clone().send((event.into(), sender)).await?;

        Ok(receiver)
    }

    pub async fn emit_responseless(self: Arc<Self>, event: impl Into<T>) -> Result<(), SendError> {
        self.sender
            .clone()
            .send((event.into(), mpsc::unbounded().0))
            .await
    }
}
