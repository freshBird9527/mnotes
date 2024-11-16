# polymorphism

## 参数多态性（Parametric Polymorphism）
```rust
trait Service {
    fn start(&self);
}

struct LogService {
    name: String,
}

impl LogService {
    fn new(name: &str) ->Self {
        LogService {
            name: name.to_string()
        }
    }
}

impl Service for LogService {
    fn start(&self) {
        println!("=== start {} ===", self.name);
    }
}

// 和下面的写法完全等价
// fn run_service(service: impl Service) {
//     service.start();
// }

fn run_service<T: Service>(service: T) {
    service.start();
}

fn main() {
    let service = LogService::new("LogService");
    run_service(service);
}
```



