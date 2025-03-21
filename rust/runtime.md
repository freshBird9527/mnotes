# runtime

## init

### 业务代码

* 实现了一个TCP的echo server;
* 每accept一个tcp链接就spawn 一个task;
* 构造了一个multi_thread的runtime;

```rust
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn echo_handler(mut stream: TcpStream) {
    let mut buf = vec![0; 4096];

    loop {
        match stream.read(&mut buf).await {
            Ok(0) => return,
            Ok(n) => {
                if stream.write_all(&buf[..n]).await.is_err() {
                    return;
                }
            }
            Err(_) => {
                return;
            }
        }
    }
}

fn main() -> io::Result<()> {
    let body = async {
        let addr = "127.0.0.1:8096";
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            tokio::spawn(async move { echo_handler(stream).await });
        }
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");

    return rt.block_on(body);
}
```

### 初始化runtime

#### runtime/builder.rs

* Builder 结构中包含很多构造runtime的参数；
* `new_multi_thread` 和 `enable_all`方法都是单纯设置Builder的成员;
* `build`方法根据`new_multi_thread` 设置的类型调用`build_threaded_runtime`方法;
```rust
pub struct Builder {
    /// Runtime type
    kind: Kind,

    /// Whether or not to enable the I/O driver
    enable_io: bool,
    nevents: usize,

    // ......

    #[cfg(tokio_unstable)]
    pub(super) unhandled_panic: UnhandledPanic,
}

impl Builder {
    #[cfg(feature = "rt-multi-thread")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt-multi-thread")))]
    pub fn new_multi_thread() -> Builder {
        // The number `61` is fairly arbitrary. I believe this value was copied from golang.
        Builder::new(Kind::MultiThread, 61)
    }

    pub fn enable_all(&mut self) -> &mut Self {
        self.enable_io();
        self.enable_time();

        self
    }

    pub fn build(&mut self) -> io::Result<Runtime> {
        match &self.kind {
            Kind::CurrentThread => self.build_current_thread_runtime(),
            #[cfg(feature = "rt-multi-thread")]
            Kind::MultiThread => self.build_threaded_runtime(),
            #[cfg(all(tokio_unstable, feature = "rt-multi-thread"))]
            Kind::MultiThreadAlt => self.build_alt_threaded_runtime(),
        }
    }
}
```

* 构造runtime的核心逻辑，根据Builer的信息构造出Runtime;
* 

```rust
impl Builder {
    fn build_threaded_runtime(&mut self) -> io::Result<Runtime> {
        use crate::loom::sys::num_cpus;
        use crate::runtime::{Config, runtime::Scheduler};
        use crate::runtime::scheduler::{self, MultiThread};

        let core_threads = self.worker_threads.unwrap_or_else(num_cpus);

        let (driver, driver_handle) = driver::Driver::new(self.get_cfg(core_threads))?;

        // Create the blocking pool
        let blocking_pool =
            blocking::create_blocking_pool(self, self.max_blocking_threads + core_threads);
        let blocking_spawner = blocking_pool.spawner().clone();

        // Generate a rng seed for this runtime.
        let seed_generator_1 = self.seed_generator.next_generator();
        let seed_generator_2 = self.seed_generator.next_generator();

        let (scheduler, handle, launch) = MultiThread::new(
            core_threads,
            driver,
            driver_handle,
            blocking_spawner,
            seed_generator_2,
            // 可见这个config信息都来自Builder结构
            Config {
                before_park: self.before_park.clone(),
                after_unpark: self.after_unpark.clone(),
                before_spawn: self.before_spawn.clone(),
                after_termination: self.after_termination.clone(),
                global_queue_interval: self.global_queue_interval,
                event_interval: self.event_interval,
                local_queue_capacity: self.local_queue_capacity,
                #[cfg(tokio_unstable)]
                unhandled_panic: self.unhandled_panic.clone(),
                disable_lifo_slot: self.disable_lifo_slot,
                seed_generator: seed_generator_1,
                metrics_poll_count_histogram: self.metrics_poll_count_histogram_builder(),
            },
        );

        // 将 Arc<runtime::scheduler::multi_thread::Handle> 转换为 runtime::scheduler::Handle枚举，然后再转换为runtime::handle::Handle;
        let handle = Handle { inner: scheduler::Handle::MultiThread(handle) };

        // Spawn the thread pool workers
        let _enter = handle.enter();
        launch.launch();

        Ok(Runtime::from_parts(Scheduler::MultiThread(scheduler), handle, blocking_pool))
    }
}
```

#### runtime/scheduler/multi_thread/mod.rs
```rust
pub(crate) struct MultiThread;

impl MultiThread {
    pub(crate) fn new(
        size: usize,
        driver: Driver,
        driver_handle: driver::Handle,
        blocking_spawner: blocking::Spawner,
        seed_generator: RngSeedGenerator,
        config: Config,
    ) -> (MultiThread, Arc<Handle>, Launch) {
        let parker = Parker::new(driver);
        let (handle, launch) = worker::create(
            size,
            parker,
            driver_handle,
            blocking_spawner,
            seed_generator,
            config,
        );

        (MultiThread, handle, launch)
    }
}
```


#### runtime/scheduler/multi_thread/worker.rs

* 根据worker的数量创建，Core、Remote、WorkerMetrics，最终只返回了handle和 Launch；
* 将 driver_handle、blocking_spawner、remotes、worker_metrics等均放入runtime共享的handle中；
* 创建的cores 会放入woker中，即launch;

```rust
pub(super) fn create(
    size: usize,
    park: Parker,
    driver_handle: driver::Handle,
    blocking_spawner: blocking::Spawner,
    seed_generator: RngSeedGenerator,
    config: Config,
) -> (Arc<Handle>, Launch) {
    // 此处的size是woker数，默认和cup数相同;
    let mut cores = Vec::with_capacity(size);
    let mut remotes = Vec::with_capacity(size);
    let mut worker_metrics = Vec::with_capacity(size);

    // Create the local queues
    for _ in 0..size {
        // 每个worker自己的local task queue，长度总是 LOCAL_QUEUE_CAPACITY == 256;
        let (steal, run_queue) = queue::local();

        let park = park.clone();
        let unpark = park.unpark();
        let metrics = WorkerMetrics::from_config(&config);
        let stats = Stats::new(&metrics);

        cores.push(Box::new(Core {
            tick: 0,
            lifo_slot: None,
            lifo_enabled: !config.disable_lifo_slot,
            run_queue,
            is_searching: false,
            is_shutdown: false,
            is_traced: false,
            park: Some(park),
            global_queue_interval: stats.tuned_global_queue_interval(&config),
            stats,
            rand: FastRand::from_seed(config.seed_generator.next_seed()),
        }));

        remotes.push(Remote { steal, unpark });
        worker_metrics.push(metrics);
    }

    let (idle, idle_synced) = Idle::new(size);
    let (inject, inject_synced) = inject::Shared::new();

    let remotes_len = remotes.len();

    // 此处 mutti_thread::handle::Handle使用了Arc，最终会被runtime::handle::Handle使用
    let handle = Arc::new(Handle {
        task_hooks: TaskHooks {
            task_spawn_callback: config.before_spawn.clone(),
            task_terminate_callback: config.after_termination.clone(),
        },
        shared: Shared {
            remotes: remotes.into_boxed_slice(),
            inject,  // 包含全局task queue的长度(原子类型);
            idle,
            owned: OwnedTasks::new(size),
            synced: Mutex::new(Synced { // 此处使用mutex保护，长度不放进去主要减少加锁范围；
                idle: idle_synced, 
                inject: inject_synced, // 包含全局task queue 双向链表头;
            }),
            shutdown_cores: Mutex::new(vec![]),
            trace_status: TraceStatus::new(remotes_len),
            config,
            scheduler_metrics: SchedulerMetrics::new(),
            worker_metrics: worker_metrics.into_boxed_slice(),
            _counters: Counters,
        },
        driver: driver_handle,
        blocking_spawner,
        seed_generator,
    });

    let mut launch = Launch(vec![]);

    for (index, core) in cores.drain(..).enumerate() {
        launch.0.push(Arc::new(Worker {
            handle: handle.clone(),
            index,
            core: AtomicCell::new(Some(core)),
        }));
    }

    (handle, launch)
}
```

* 使用OS级线程创建worker，下面的run函数将在worker线程中执行；
* 暂时不分析OS级别线程的创建流程，只需明确run函数在新的OS线程执行即可；
* run函数在worker线程内调用Context::run方法循环执行任务;

```rust
impl Launch {
    pub(crate) fn launch(mut self) {
        for worker in self.0.drain(..) {
            runtime::spawn_blocking(move || run(worker)); // 参见阻塞线程相关逻辑
        }
    }
}

fn run(worker: Arc<Worker>) {
    #[allow(dead_code)]
    struct AbortOnPanic;

    impl Drop for AbortOnPanic {
        fn drop(&mut self) {
            if std::thread::panicking() {
                eprintln!("worker thread panicking; aborting process");
                std::process::abort();
            }
        }
    }

    // Catching panics on worker threads in tests is quite tricky. Instead, when
    // debug assertions are enabled, we just abort the process.
    #[cfg(debug_assertions)]
    let _abort_on_panic = AbortOnPanic;

    // Acquire a core. If this fails, then another thread is running this
    // worker and there is nothing further to do.
    let core = match worker.core.take() {
        Some(core) => core,
        None => return,
    };

    worker.handle.shared.worker_metrics[worker.index].set_thread_id(thread::current().id());

    let handle = scheduler::Handle::MultiThread(worker.handle.clone());

    crate::runtime::context::enter_runtime(&handle, true, |_| {
        // Set the worker context.
        let cx = scheduler::Context::MultiThread(Context {
            worker,
            core: RefCell::new(None),
            defer: Defer::new(),
        });

        context::set_scheduler(&cx, || {
            let cx = cx.expect_multi_thread();

            // This should always be an error. It only returns a `Result` to support
            // using `?` to short circuit.
            assert!(cx.run(core).is_err()); //  worker内循环的执行task 

            // Check if there are any deferred tasks to notify. This can happen when
            // the worker core is lost due to `block_in_place()` being called from
            // within the task.
            cx.defer.wake();
        });
    });
}


impl Context {
    fn run(&self, mut core: Box<Core>) -> RunResult {
        // Reset `lifo_enabled` here in case the core was previously stolen from
        // a task that had the LIFO slot disabled.
        self.reset_lifo_enabled(&mut core);

        // Start as "processing" tasks as polling tasks from the local queue
        // will be one of the first things we do.
        core.stats.start_processing_scheduled_tasks();

        while !core.is_shutdown {
            self.assert_lifo_enabled_is_correct(&core);

            if core.is_traced {
                core = self.worker.handle.trace_core(core);
            }

            // Increment the tick
            core.tick();

            // Run maintenance, if needed
            core = self.maintenance(core);

            // First, check work available to the current worker.
            if let Some(task) = core.next_task(&self.worker) {
                core = self.run_task(task, core)?;
                continue;
            }

            // We consumed all work in the queues and will start searching for work.
            core.stats.end_processing_scheduled_tasks();

            // There is no more **local** work to process, try to steal work
            // from other workers.
            if let Some(task) = core.steal_work(&self.worker) {
                // Found work, switch back to processing
                core.stats.start_processing_scheduled_tasks();
                core = self.run_task(task, core)?;
            } else {
                // Wait for work
                core = if !self.defer.is_empty() {
                    self.park_timeout(core, Some(Duration::from_millis(0)))
                } else {
                    self.park(core)
                };
                core.stats.start_processing_scheduled_tasks();
            }
        }

        core.pre_shutdown(&self.worker);
        // Signal shutdown
        self.worker.handle.shutdown_core(core);
        Err(())
    }
}
```

##### 数据结构

```rust
struct Core {
    /// Used to schedule bookkeeping tasks every so often.
    tick: u32,

    /// When a task is scheduled from a worker, it is stored in this slot. The
    /// worker will check this slot for a task **before** checking the run
    /// queue. This effectively results in the **last** scheduled task to be run
    /// next (LIFO). This is an optimization for improving locality which
    /// benefits message passing patterns and helps to reduce latency.
    lifo_slot: Option<Notified>,

    /// When `true`, locally scheduled tasks go to the LIFO slot. When `false`,
    /// they go to the back of the `run_queue`.
    lifo_enabled: bool,

    /// The worker-local run queue.
    run_queue: queue::Local<Arc<Handle>>,

    /// True if the worker is currently searching for more work. Searching
    /// involves attempting to steal from other workers.
    is_searching: bool,

    /// True if the scheduler is being shutdown
    is_shutdown: bool,

    /// True if the scheduler is being traced
    is_traced: bool,

    /// Parker
    ///
    /// Stored in an `Option` as the parker is added / removed to make the
    /// borrow checker happy.
    park: Option<Parker>,

    /// Per-worker runtime stats
    stats: Stats,

    /// How often to check the global queue
    global_queue_interval: u32,

    /// Fast random number generator.
    rand: FastRand,
}

pub(super) struct Worker {
    // 关联到 scheduler::mutti_thread::handle::Handle
    handle: Arc<Handle>,

    /// Index holding this worker's remote state
    index: usize,

    /// Used to hand-off a worker's core to another thread.
    core: AtomicCell<Core>,
}

/// Starts the workers
pub(crate) struct Launch(Vec<Arc<Worker>>);

/// Used to communicate with a worker from other threads.
struct Remote {
    /// Steals tasks from this worker.
    pub(super) steal: queue::Steal<Arc<Handle>>,

    /// Unparks the associated worker thread
    unpark: Unparker,
}

/// State shared across all workers
pub(crate) struct Shared {
    /// Per-worker remote state. All other workers have access to this and is
    /// how they communicate between each other.
    remotes: Box<[Remote]>,

    /// Global task queue used for:
    ///  1. Submit work to the scheduler while **not** currently on a worker thread.
    ///  2. Submit work to the scheduler when a worker run queue is saturated
    pub(super) inject: inject::Shared<Arc<Handle>>,

    /// Coordinates idle workers
    idle: Idle,

    /// Collection of all active tasks spawned onto this executor.
    pub(crate) owned: OwnedTasks<Arc<Handle>>,

    /// Data synchronized by the scheduler mutex
    pub(super) synced: Mutex<Synced>,

    /// Cores that have observed the shutdown signal
    ///
    /// The core is **not** placed back in the worker to avoid it from being
    /// stolen by a thread that was spawned as part of `block_in_place`.
    #[allow(clippy::vec_box)] // we're moving an already-boxed value
    shutdown_cores: Mutex<Vec<Box<Core>>>,

    /// The number of cores that have observed the trace signal.
    pub(super) trace_status: TraceStatus,

    /// Scheduler configuration options
    config: Config,

    /// Collects metrics from the runtime.
    pub(super) scheduler_metrics: SchedulerMetrics,

    pub(super) worker_metrics: Box<[WorkerMetrics]>,

    /// Only held to trigger some code on drop. This is used to get internal
    /// runtime metrics that can be useful when doing performance
    /// investigations. This does nothing (empty struct, no drop impl) unless
    /// the `tokio_internal_mt_counters` `cfg` flag is set.
    _counters: Counters,
}

// runtime/scheduler/multi_thread/handle.rs: Handle to the multi thread scheduler
pub(crate) struct Handle {
    /// Task spawner
    pub(super) shared: worker::Shared,

    /// Resource driver handles
    pub(crate) driver: driver::Handle,

    /// Blocking pool spawner
    pub(crate) blocking_spawner: blocking::Spawner,

    /// Current random number generator seed
    pub(crate) seed_generator: RngSeedGenerator,

    /// User-supplied hooks to invoke for things
    pub(crate) task_hooks: TaskHooks,
}
```


## scheduler

* 并没有一个独立的线程来处理所有IO事件，而是在哪个woker拿到了driver就由那个worker来分发IO事件（epoll_wait），没拿到driver的worker则阻塞到条件变量；

### park

#### runtime/scheduler/multi_thread/worker.rs
* 当worker 没有任务执行时就会调用park进入睡眠，参见runtime::scheduler::multi_thread::worker::Context::run;

```rust
impl Context {
    fn park_timeout(&self, mut core: Box<Core>, duration: Option<Duration>) -> Box<Core> {
        self.assert_lifo_enabled_is_correct(&core);

        // Take the parker out of core
        let mut park = core.park.take().expect("park missing");

        // Store `core` in context
        *self.core.borrow_mut() = Some(core);

        // Park thread
        if let Some(timeout) = duration {
            park.park_timeout(&self.worker.handle.driver, timeout);
        } else {
            park.park(&self.worker.handle.driver);
        }

        self.defer.wake();

        // Remove `core` from context
        core = self.core.borrow_mut().take().expect("core missing");

        // Place `park` back in `core`
        core.park = Some(park);

        if core.should_notify_others() {
            self.worker.handle.notify_parked_local();
        }

        core
    }
}
```

#### runtime/scheduler/multi_thread/park.rs

* 所有的woker持有相同的 Inner.shared参见runtime/scheduler/multi_thread/mod.rs::MultiThread::new()；
* Inner::park中拿到self.shared.driver时，则最终会epoll_wait进行事件分发；
* Inner::park中未拿到self.shared.driver时，最终会阻塞到linux的多线程条件变量上；
* 每个worker有自己的condvar，并非所有线程都阻塞在同一个条件变量，参见 `impl Clone for Parker`及`Inner::park_condvar`;
* `mutti_thread::handle::Handle` 持有的`remotes`包含了`Unparker`，参见runtime/scheduler/multi_thread/worker.rs;
* worker线程会使用下面的Parker;

```rust
pub(crate) struct Parker {
    inner: Arc<Inner>,
}

pub(crate) struct Unparker {
    inner: Arc<Inner>,
}

/// Shared across multiple Parker handles
struct Shared {
    /// Shared driver. Only one thread at a time can use this
    driver: TryLock<Driver>,
}

struct Inner {
    /// Avoids entering the park if possible
    state: AtomicUsize,

    /// Used to coordinate access to the driver / `condvar`
    mutex: Mutex<()>,

    /// `Condvar` to block on if the driver is unavailable.
    condvar: Condvar,

    /// Resource (I/O, time, ...) driver
    shared: Arc<Shared>,
}


impl Parker {
    pub(crate) fn unpark(&self) -> Unparker {
        Unparker {
            inner: self.inner.clone(),
        }
    }

    pub(crate) fn park(&mut self, handle: &driver::Handle) {
        self.inner.park(handle);
    }
}


impl Clone for Parker {
    fn clone(&self) -> Parker {
        Parker {
            inner: Arc::new(Inner {
                state: AtomicUsize::new(EMPTY),
                mutex: Mutex::new(()),
                condvar: Condvar::new(),   // 阻塞在自己的条件变量上
                shared: self.inner.shared.clone(), // shared成员driver所有worker共享;
            }),
        }
    }
}

impl Unparker {
    pub(crate) fn unpark(&self, driver: &driver::Handle) {
        self.inner.unpark(driver);
    }
}

impl Inner {
    /// Parks the current thread for at most `dur`.
    fn park(&self, handle: &driver::Handle) {
        // If we were previously notified then we consume this notification and
        // return quickly.
        if self
            .state
            .compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst)
            .is_ok()
        {
            return;
        }

        if let Some(mut driver) = self.shared.driver.try_lock() {
            self.park_driver(&mut driver, handle); // TryLock 使得只有一个线程能获取到driver
        } else {
            self.park_condvar();
        }
    }

    fn unpark(&self, driver: &driver::Handle) {
        // To ensure the unparked thread will observe any writes we made before
        // this call, we must perform a release operation that `park` can
        // synchronize with. To do that we must write `NOTIFIED` even if `state`
        // is already `NOTIFIED`. That is why this must be a swap rather than a
        // compare-and-swap that returns if it reads `NOTIFIED` on failure.
        match self.state.swap(NOTIFIED, SeqCst) {
            EMPTY => {}    // no one was waiting
            NOTIFIED => {} // already unparked
            PARKED_CONDVAR => self.unpark_condvar(),
            PARKED_DRIVER => driver.unpark(),
            actual => panic!("inconsistent state in unpark; actual = {actual}"),
        }
    }

    fn park_driver(&self, driver: &mut Driver, handle: &driver::Handle) {
        match self
            .state
            .compare_exchange(EMPTY, PARKED_DRIVER, SeqCst, SeqCst)
        {
            Ok(_) => {}
            Err(NOTIFIED) => {
                // We must read here, even though we know it will be `NOTIFIED`.
                // This is because `unpark` may have been called again since we read
                // `NOTIFIED` in the `compare_exchange` above. We must perform an
                // acquire operation that synchronizes with that `unpark` to observe
                // any writes it made before the call to unpark. To do that we must
                // read from the write it made to `state`.
                let old = self.state.swap(EMPTY, SeqCst);
                debug_assert_eq!(old, NOTIFIED, "park state changed unexpectedly");

                return;
            }
            Err(actual) => panic!("inconsistent park state; actual = {actual}"),
        }

        driver.park(handle);

        match self.state.swap(EMPTY, SeqCst) {
            NOTIFIED => {}      // got a notification, hurray!
            PARKED_DRIVER => {} // no notification, alas
            n => panic!("inconsistent park_timeout state: {n}"),
        }
    }

}
```

#### runtime/io/scheduler_io.rs

* tokio::runtime::scheduler::multi_thread::park::Inner::park_driver 调用链中，当epoll_wait返回时会执行tokio::runtime::io::scheduler_io::ScheduledIo::wake;
* tokio::runtime::io::scheduler_io::ScheduledIo::wake 最终会执行 `tokio::runtime::park::Inner::unpark`;
* 主线程并不会使用 `tokio::runtime::scheduler::multi_thread::park::Parker::park` 而是使用 `tokio::runtime::park::Inner::park`;

```rust
impl ScheduledIo {

    /// Notifies all pending waiters that have registered interest in `ready`.
    ///
    /// There may be many waiters to notify. Waking the pending task **must** be
    /// done from outside of the lock otherwise there is a potential for a
    /// deadlock.
    ///
    /// A stack array of wakers is created and filled with wakers to notify, the
    /// lock is released, and the wakers are notified. Because there may be more
    /// than 32 wakers to notify, if the stack array fills up, the lock is
    /// released, the array is cleared, and the iteration continues.
    pub(super) fn wake(&self, ready: Ready) {
        let mut wakers = WakeList::new();

        let mut waiters = self.waiters.lock();

        // check for AsyncRead slot
        if ready.is_readable() {
            if let Some(waker) = waiters.reader.take() {
                wakers.push(waker);
            }
        }

        // check for AsyncWrite slot
        if ready.is_writable() {
            if let Some(waker) = waiters.writer.take() {
                wakers.push(waker);
            }
        }

        'outer: loop {
            let mut iter = waiters.list.drain_filter(|w| ready.satisfies(w.interest));

            while wakers.can_push() {
                match iter.next() {
                    Some(waiter) => {
                        let waiter = unsafe { &mut *waiter.as_ptr() };

                        if let Some(waker) = waiter.waker.take() {
                            waiter.is_ready = true;
                            wakers.push(waker);
                        }
                    }
                    None => {
                        break 'outer;
                    }
                }
            }

            drop(waiters);

            wakers.wake_all();

            // Acquire the lock again.
            waiters = self.waiters.lock();
        }

        // Release the lock before notifying
        drop(waiters);

        wakers.wake_all();
    }
}
```



## TODO
* 分析 remotes中两个成员的作用；
* 分析 crate::runtime::context::enter_runtime等各种与CONTEXT的作用；
* 梳理各种handle context关系；
* 梳理IO等待的关系；
* 梳理master线程得block_on阻塞；