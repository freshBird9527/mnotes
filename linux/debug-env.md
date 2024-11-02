# debug-env

* linux-5.15.170


## 编译内核

### 安装依赖
```shell
sudo apt install build-essential libncurses-dev bison flex libssl-dev libelf-dev dwarves
```

### 配置

#### 生成

```shell
make ARCH=x86_64 x86_64_defconfig

# 将.config文件中:
# CONFIG_DEBUG_INFO is not set

# 替换为:
CONFIG_DEBUG_INFO=y
# CONFIG_DEBUG_INFO_REDUCED is not set
CONFIG_DEBUG_INFO_COMPRESSED=y
# CONFIG_DEBUG_INFO_SPLIT is not set
CONFIG_DEBUG_INFO_DWARF_TOOLCHAIN_DEFAULT=y
# CONFIG_DEBUG_INFO_DWARF4 is not set
CONFIG_DEBUG_INFO_BTF=y
CONFIG_GDB_SCRIPTS=y
```

#### 清理
```shell
make mrproper
```

### 编译

```shell
make -j4
# 生成编译命令 compile_commands.json
./scripts/clang-tools/gen_compile_commands.py
```

## 制作rootfs

### 编译busybox

* busybox-1.36.1

```shell
make menuconfig

# 静态编译
Settings  --->
	--- Build Options
	[*] Build static binary (no shared libs)

# 新内核编译不过时
--- Applets
	Networking Utilities  --->
		[ ] tc (8.3 kb)

make -j4

# 安装到当前目录 _install/
make install 
```

### 制作initramfs
```shell
mkdir -p initramfs/{bin,dev,etc,home,mnt,proc,sys,usr}
cd initramfs/dev
sudo mknod sda b 8 0 
sudo mknod console c 5 1

cd ..
cp -r ../busybox-1.36.1/_install/* ./

# 创建init脚本
vim init
chmod +x init
```

#### init 脚本
```shell
#!/bin/sh
mount -t proc none /proc
mount -t sysfs none /sys
exec /bin/sh
```

#### 打包
```shell
find . -print0 | cpio --null -ov --format=newc > initramfs.cpio
gzip ./initramfs.cpio
# 生成 initramfs.cpio.gz
```

## 使用qemu运行

### 安装
```shell
sudo apt install qemu-system-x86
```

### 运行脚本

```shell
#!/bin/bash

qemu-system-x86_64 \
	-kernel ./bzImage \
	-initrd ./initramfs.cpio.gz \
	-append "console=ttyS0 nokaslr" \
	-serial stdio \
	-display none \
	-m 200M \
	-s \
	-S
```

### gdb

```shell
gdb vmlinux
(gdb) target remote :1234
```

