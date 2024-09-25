# ReadMe

## 描述

### services::Servic

```rust
#[async_trait]
pub trait Service: Sync + Send {
	// 当执行server.run_forever()时为每个servie创建独立的运行时，该函数在独立的运行时中被执行；
    async fn start_service(&mut self, fds: Option<ListenFds>, mut shutdown: ShutdownWatch);

    fn name(&self) -> &str;

    // If `None`, the global setting will be used
    fn threads(&self) -> Option<usize> {
        None
    }
}
```

* server::Server会包含该trait;
* services::listening::Servic 实现该trait的核心逻辑是为每个监听fd创建一个task，然后在task内listen->accept->;
