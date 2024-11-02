# bootloader

## boot

* BIOS 运行结束时，从0x7c00开始执行，引导程序被BIOS加载该地址；
* boot程序，操作屏幕、读磁盘都需要借助BIOS的中断处理程序；










## loader

### big-real-mode

* big-real-mode 能访问4G的内存空间；
* 在big-real-mode下修改fs段寄存器，会重新回到real-mode;

```asm
; enable_a20_fast
push   ax
in     al,  92h            ; 从IO端口(92h)读一个字节到al寄存器
or     al,  00000010b      ; 将al寄存器的bit 1置为 1
out    92h, al             ; 将al寄存器写入IO端口(92h)
pop    ax

cli                        ; 关闭外部中断,因为未配置IDT

; init GDTR
db      0x66               ; lgdt指令前缀，操作数从 16 位扩展为 32 位
lgdt    [Gdtptr]           ; 32位地址(保护模式)，而非16位地址

; switch to protected-mode
mov    eax,  cr0
or     eax,  00000001b
mov    cr0,  eax

; exist protected-mode
mov    ax, Selector
mov    fs,  ax              ; 将选择器设置到fs 段寄存器
mov    eax, cr0             
and    al,  11111110b       ; 准备退出保护模式
mov    cr0,  eax

sti                         ; 开启外部中断 
```



## 汇编

* callq 指令会将当前的返回地址（即下一条指令的地址）压入栈中，然后跳转到指定的函数地址执行。
* retq 指令会从栈顶弹出一个返回地址，并跳转到该地址;



## A20
* 控制地址回绕，未开启时访问超过1M的内存就会回绕，开启后不会回绕；
* 实模式下，开启A20最大能访问到(0xFFFF << 4) + 0xFFFF = 0x10FFEF;
* 




IA32_EFER:

IA32_KERNEL_GS_BASE — Used by SWAPGS instruction.
• IA32_LSTAR — Used by SYSCALL instruction.
• IA32_FMASK — Used by SYSCALL instruction.
• IA32_STAR — Used by SYSCALL and SYSRET instruction.


void syscall_init(void)
{
	wrmsr(MSR_STAR, 0, (__USER32_CS << 16) | __KERNEL_CS);
	wrmsrl(MSR_LSTAR, (unsigned long)entry_SYSCALL_64);

