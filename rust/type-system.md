# 类型系统

## 待整理
类型，是对值的区分，它包含了值在内存中的长度、对齐以及值可以进行的操作等信息。
按定义后类型是否可以隐式转换，可以分为强类型和弱类型。Rust 不同类型间不能自动转换，所以是强类型语言，而 C/C++/JavaScript 会自动转换，是弱类型语言。
按类型检查的时机，在编译时检查还是运行时检查，可以分为静态类型系统和动态类型系统。
从内存的角度看，类型安全是指代码，只能按照被允许的方法，访问它被授权访问的内存（没有c/c++的隐式转换）。
在此基础上，Rust 还进一步对内存的访问进行了读/写分开的授权。所以，Rust 下的内存安全更严格：代码只能按照被允许的方法和被允许的权限，访问它被授权访问的内存。
为了做到这么严格的类型安全，Rust 中除了 let/fn/static/const 这些定义性语句外，都是表达式，而一切表达式都有类型，所以可以说在 Rust 中，类型无处不在。


表达式是计算值的代码片段。它可以包含变量、常量、运算符和函数调用，并且最终会产生一个结果(表达式总是返回一个值)。
语句是执行某个操作的指令。语句本身不返回值，但可能会改变程序的状态或执行某些动作。(let x=5为赋值语句, if con {}为条件语句)










