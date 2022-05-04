//#[allow(unused)]
use {
    futures::{
        future::{BoxFuture, FutureExt},
        task::{waker_ref, ArcWake},
    },
    std::{
        future::Future,
        sync::mpsc::{sync_channel, Receiver, SyncSender},
        sync::{Arc, Mutex},
        task::Context,
    }
};

/// Task executor that receives tasks off of a channel and runs them.
pub struct Executor {
    ready_queue: Receiver<Arc<Task>>,
}

/// `Spawner` spawns new futures onto the task channel.
#[derive(Clone)]
pub struct Spawner {
    task_sender: SyncSender<Arc<Task>>,
}

/// A future that can reschedule itself to be polled by an `Executor`.
pub struct Task {
    /// In-progress future that should be pushed to completion.
    ///
    /// The `Mutex` is not necessary for correctness, since we only have
    /// one thread executing tasks at once. However, Rust isn't smart
    /// enough to know that `future` is only mutated from one thread,
    /// so we need to use the `Mutex` to prove thread-safety. A production
    /// executor would not need this, and could use `UnsafeCell` instead.
    future: Mutex<Option<BoxFuture<'static, ()>>>,

    /// Handle to place the task itself back onto the task queue.
    task_sender: SyncSender<Arc<Task>>,
}

pub fn new_executor_and_spawner() -> (Executor, Spawner) {
    // Maximum number of tasks to allow queueing in the channel at once.
    // This is just to make `sync_channel` happy, and wouldn't be present in
    // a real executor.
    log::trace!("new_executor_and_spawner:+");

    const MAX_QUEUED_TASKS: usize = 10_000;
    let (task_sender, ready_queue) = sync_channel(MAX_QUEUED_TASKS);

    let res = (Executor { ready_queue }, Spawner { task_sender });
    log::trace!("new_executor_and_spawner:-");
    res
}

impl Spawner {
    // This probably needs to return a `impl future<Output = ()>`
    // so the "task" spawned cand be await'd or polled????
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        log::trace!("spawn:+");

        let future = future.boxed();
        let task = Arc::new(Task {
            future: Mutex::new(Some(future)),
            task_sender: self.task_sender.clone(),
        });

        self.task_sender.send(task).expect("too many tasks queued");
        log::trace!("spawn:-");
    }
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        log::trace!("Task::wake_by_ref:+");
        // Implement `wake` by sending this task back onto the task channel
        // so that it will be polled again by the executor.
        let cloned = arc_self.clone();
        arc_self
            .task_sender
            .send(cloned)
            .expect("too many tasks queued");
        log::trace!("Task::wake_by_ref:-");
    }
}

impl Executor {
    pub fn run(&self) {
        log::trace!("Executor::run:+");
        while let Ok(task) = self.ready_queue.recv() {
            log::trace!("Executor::run: TOL");
            // Take the future, and if it has not yet completed (is still Some),
            // poll it in an attempt to complete it.
            let mut future_slot = task.future.lock().unwrap();
            if let Some(mut future) = future_slot.take() {
                log::trace!("Executor::run: waker_ref+");
                // Create a `LocalWaker` from the task itself
                let waker = waker_ref(&task);
                log::trace!("Executor::run: waker_ref-; context::from_waker+");
                let context = &mut Context::from_waker(&*waker);
                log::trace!("Executor::run: context::from_waker-");
                // `BoxFuture<T>` is a type alias for
                // `Pin<Box<dyn Future<Output = T> + Send + 'static>>`.
                // We can get a `Pin<&mut dyn Future + Send + 'static>`
                // from it by calling the `Pin::as_mut` method.
                log::trace!("Executor::run: poll(cx)+");
                if future.as_mut().poll(context).is_pending() {
                    log::trace!("Executor::run: poll(cx)- is_pending(); put back Future, its not done");
                    // We're not done processing the future, so put it
                    // back in its task to be run again in the future.
                    *future_slot = Some(future);
                } else {
                    log::trace!("Executor::run: poll(cx)- Future is done");
                }
            }
            log::trace!("Executor::run: BOL");
        }
        log::trace!("Executor::run:-");
    }
}