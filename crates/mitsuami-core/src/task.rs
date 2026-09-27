//! Async on the UI thread: a small executor driven by [`Ui::tick`], timers
//! on a swappable clock, and a bridge for work done on other threads.
//!
//! - [`spawn_local`] runs a future on the UI thread. It is owned by the
//!   current reactive scope and cancelled when that scope is disposed.
//! - [`spawn_blocking`] runs a closure on a background thread and resolves
//!   (on the UI thread) with its result.
//! - [`sleep`] waits on the `Ui`'s clock: real time in apps, a manual clock in
//!   tests.
//!
//! [`Ui::tick`]: crate::Ui::tick

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use mitsuami_reactive::Owner;

use crate::ui::Ui;

type LocalFuture = Pin<Box<dyn Future<Output = ()>>>;

/// A task and the reactive scope it runs in.
struct Task {
    future: LocalFuture,
    owner: Option<Owner>,
}

thread_local! {
    /// The Ui that is running code right now (polling tasks, dispatching
    /// events), for code with no reactive scope to find it in.
    static CURRENT: RefCell<Option<crate::ui::WeakUi>> = const { RefCell::new(None) };
}

/// Runs `f` with `ui` as the current Ui.
pub(crate) fn with_current<R>(ui: &Ui, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|c| c.replace(Some(ui.downgrade())));
    let result = f();
    CURRENT.with(|c| *c.borrow_mut() = previous);
    result
}

/// Time as the UI sees it. Apps use real time; tests use a manual clock.
pub trait Clock {
    /// Time elapsed since the clock started.
    fn now(&self) -> Duration;
}

/// Real, monotonic time.
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> SystemClock {
        SystemClock(Instant::now())
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// A clock that only moves when told to. Share it with [`Rc`] to advance it.
#[derive(Default)]
pub struct ManualClock(Cell<Duration>);

impl ManualClock {
    pub fn advance(&self, by: Duration) {
        self.0.set(self.0.get() + by);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
}

impl Clock for Rc<ManualClock> {
    fn now(&self) -> Duration {
        self.0.get()
    }
}

/// Where wakers put ready tasks. Shared with other threads.
#[derive(Default)]
struct ReadyQueue {
    ready: Mutex<VecDeque<u64>>,
    /// Asks the UI thread's run loop to turn (thread-safe).
    wake_ui: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

struct TaskWaker {
    id: u64,
    queue: Arc<ReadyQueue>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.queue.ready.lock().unwrap().push_back(self.id);
        let wake_ui = self.queue.wake_ui.lock().unwrap().clone();
        if let Some(wake_ui) = wake_ui {
            wake_ui();
        }
    }
}

/// The executor owned by a [`Ui`].
pub(crate) struct Executor {
    tasks: RefCell<BTreeMap<u64, Task>>,
    next_task: Cell<u64>,
    queue: Arc<ReadyQueue>,
    clock: RefCell<Rc<dyn Clock>>,
    /// Deadline → wakers of the futures sleeping until then.
    timers: RefCell<BTreeMap<(Duration, u64), Waker>>,
    next_timer: Cell<u64>,
}

impl Default for Executor {
    fn default() -> Executor {
        Executor {
            tasks: RefCell::default(),
            next_task: Cell::new(0),
            queue: Arc::default(),
            clock: RefCell::new(Rc::new(SystemClock::default())),
            timers: RefCell::default(),
            next_timer: Cell::new(0),
        }
    }
}

impl Executor {
    /// Every poll of the task runs inside `owner`, the scope it was spawned
    /// from: `inject`, `sleep` and nested `spawn_local` work as in the
    /// component, and what the task creates belongs to it.
    pub(crate) fn spawn(&self, future: LocalFuture, owner: Option<Owner>) -> u64 {
        let id = self.next_task.get();
        self.next_task.set(id + 1);
        self.tasks.borrow_mut().insert(id, Task { future, owner });
        self.queue.ready.lock().unwrap().push_back(id);
        id
    }

    pub(crate) fn cancel(&self, id: u64) {
        // Dropped outside the borrow: a future's drop may touch the executor.
        let task = self.tasks.borrow_mut().remove(&id);
        drop(task);
    }

    pub(crate) fn set_clock(&self, clock: Rc<dyn Clock>) {
        *self.clock.borrow_mut() = clock;
    }

    pub(crate) fn now(&self) -> Duration {
        self.clock.borrow().now()
    }

    pub(crate) fn set_wake_ui(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        *self.queue.wake_ui.lock().unwrap() = Some(wake);
    }

    fn add_timer(&self, deadline: Duration, waker: Waker) -> u64 {
        let id = self.next_timer.get();
        self.next_timer.set(id + 1);
        self.timers.borrow_mut().insert((deadline, id), waker);
        id
    }

    fn remove_timer(&self, deadline: Duration, id: u64) {
        self.timers.borrow_mut().remove(&(deadline, id));
    }

    /// When the earliest timer fires, if any.
    pub(crate) fn next_deadline(&self) -> Option<Duration> {
        self.timers.borrow().keys().next().map(|(deadline, _)| *deadline)
    }

    /// Fires due timers and polls ready tasks until nothing is ready.
    /// Returns whether any task ran.
    pub(crate) fn run_ready(&self, ui: &Ui) -> bool {
        with_current(ui, || self.poll_ready())
    }

    fn poll_ready(&self) -> bool {
        let now = self.now();
        let due: Vec<Waker> = {
            let mut timers = self.timers.borrow_mut();
            let later = timers.split_off(&(now, u64::MAX));
            std::mem::replace(&mut *timers, later).into_values().collect()
        };
        for waker in due {
            waker.wake();
        }
        let mut ran = false;
        loop {
            let Some(id) = self.queue.ready.lock().unwrap().pop_front() else { break };
            // Take the task out while polling: it may spawn or cancel tasks.
            let Some(mut task) = self.tasks.borrow_mut().remove(&id) else { continue };
            ran = true;
            let waker = Waker::from(Arc::new(TaskWaker { id, queue: self.queue.clone() }));
            let mut cx = Context::from_waker(&waker);
            let pending = match task.owner {
                Some(owner) if owner.is_alive() => owner.with(|| task.future.as_mut().poll(&mut cx)),
                _ => task.future.as_mut().poll(&mut cx),
            }
            .is_pending();
            if pending {
                self.tasks.borrow_mut().insert(id, task);
            }
        }
        ran
    }

    pub(crate) fn has_ready(&self) -> bool {
        !self.queue.ready.lock().unwrap().is_empty()
    }

    pub(crate) fn task_count(&self) -> usize {
        self.tasks.borrow().len()
    }
}

/// Handle to a task started with [`spawn_local`].
#[derive(Clone)]
pub struct TaskHandle {
    id: u64,
    ui: crate::ui::WeakUi,
}

impl TaskHandle {
    pub(crate) fn new(id: u64, ui: crate::ui::WeakUi) -> TaskHandle {
        TaskHandle { id, ui }
    }

    /// Stops the task: its future is dropped and never polled again.
    pub fn cancel(&self) {
        if let Some(ui) = self.ui.upgrade() {
            ui.executor().cancel(self.id);
        }
    }
}

pub(crate) fn current_ui() -> Ui {
    CURRENT.with(|c| c.borrow().as_ref().and_then(|ui| ui.upgrade())).or_else(mitsuami_reactive::inject::<Ui>).expect(
        "mitsuami: no Ui in scope. spawn_local/sleep must run inside a mounted view or a task \
             (or use Ui::spawn_local / Ui::sleep directly)",
    )
}

/// Runs `future` on the UI thread. It is cancelled when the current reactive
/// scope (e.g. the component that started it) is disposed.
pub fn spawn_local(future: impl Future<Output = ()> + 'static) -> TaskHandle {
    let handle = current_ui().spawn_in(future, Owner::current());
    let cancel = handle.clone();
    mitsuami_reactive::on_cleanup(move || cancel.cancel());
    handle
}

/// Runs `work` on a background thread; the returned future resolves on the
/// UI thread with its result.
pub fn spawn_blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = T> {
    struct Shared<T> {
        result: Option<T>,
        waker: Option<Waker>,
    }
    let shared = Arc::new(Mutex::new(Shared { result: None, waker: None }));
    let worker = shared.clone();
    std::thread::spawn(move || {
        let result = work();
        let waker = {
            let mut shared = worker.lock().unwrap();
            shared.result = Some(result);
            shared.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    });
    std::future::poll_fn(move |cx| {
        let mut shared = shared.lock().unwrap();
        match shared.result.take() {
            Some(result) => Poll::Ready(result),
            None => {
                shared.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    })
}

/// Waits `duration` on the current `Ui`'s clock.
pub fn sleep(duration: Duration) -> Sleep {
    current_ui().sleep(duration)
}

/// Future returned by [`sleep`].
pub struct Sleep {
    ui: Ui,
    deadline: Duration,
    timer: Option<u64>,
}

impl Sleep {
    pub(crate) fn new(ui: Ui, duration: Duration) -> Sleep {
        let deadline = ui.executor().now() + duration;
        Sleep { ui, deadline, timer: None }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = &mut *self;
        let executor = this.ui.executor();
        if let Some(id) = this.timer.take() {
            executor.remove_timer(this.deadline, id);
        }
        if executor.now() >= this.deadline {
            return Poll::Ready(());
        }
        this.timer = Some(executor.add_timer(this.deadline, cx.waker().clone()));
        Poll::Pending
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(id) = self.timer.take() {
            self.ui.executor().remove_timer(self.deadline, id);
        }
    }
}
