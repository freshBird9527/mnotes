# Pin

## desc
```
Notice that the thing wrapped by Pin is not the value which we want to pin itself, but rather a pointer to that value! A Pin<Ptr> does not pin the Ptr; instead, it pins the pointer’s pointee value.

Note that this invariant is enforced by simply making it impossible to call code that would perform a move on the pinned value. This is the case since the only way to access that pinned value is through the pinning Pin<&mut T>>, which in turn restricts our access.

When T: Unpin, Pin<Box<T>> functions identically to a non-pinning Box<T>; similarly, Pin<&mut T> would impose no additional restrictions above a regular &mut T.


```

## pin 投影移除字段




## pin!()

```rust
fn main() {
    ref_t0();
    ref_t1();
}

fn ref_t0() {
    let v0 = vec![1, 2, 3];
    {
        let v1 = &mut { v0 };
        v1.push(66);
        println!("{}", v1.len());
    }

    // error[E0382]: borrow of moved value: `v0`
    // println!("{}", v0.len());
}

fn ref_t1() {
    let mut v0 = vec![1, 2, 3]; // Must declared as mutable
    {
        let v1 = &mut v0;
        v1.push(66);
        println!("{}", v1.len());
    }
    println!("{}", v0.len());
}
```