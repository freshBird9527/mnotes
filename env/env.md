# env

## nextcloud:

### docker-compose.yaml:

```shell
services:
  db:
    image: mariadb:10.11
    restart: always
    command: --transaction-isolation=READ-COMMITTED --log-bin=binlog --binlog-format=ROW
    volumes:
      - ./db:/var/lib/mysql
    environment:
      - MYSQL_ROOT_PASSWORD=db-password
      - MYSQL_PASSWORD=db-password
      - MYSQL_DATABASE=nextcloud
      - MYSQL_USER=nextcloud

  redis:
    image: redis:alpine
    restart: always

  app:
    image: nextcloud:fpm
    restart: always
    depends_on:
      - redis
      - db
    volumes:
      - /mnt/pext4/nextcloud:/var/www/html
    environment:
      - MYSQL_PASSWORD=db-password
      - MYSQL_DATABASE=nextcloud
      - MYSQL_USER=nextcloud
      - MYSQL_HOST=db

  web:
    image: nginx
    restart: always
    ports:
      - 8080:80
    depends_on:
      - app
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
    volumes_from:
      - app
```

### nginx.conf:

```
https://github.com/nextcloud/docker/blob/master/.examples/docker-compose/with-nginx-proxy/mariadb/fpm/web/nginx.conf
```

## samba:

### 创建用户

```shell
# 密码: smb-password

# 添加用户
sudo useradd smb
sudo passwd smb

sudo smbpasswd -a smb
```

### /etc/samba/smb.conf

```shell
[smb-meta]
path = /mnt/smb/private
browsable = no
writable = yes
guest ok = no
create mask = 0755

[smb-media]
path = /mnt/smb/media
browsable = yes
writable = yes
guest ok = yes
create mask = 0755
```

### 配置生效
```shell
sudo systemctl start smbd
sudo systemctl enable smbd
```

## wireguard组网

### 生成KEY
```shell
wg genpsk > preSharedKey
wg genkey | tee privateKey | wg pubkey > publicKey
```

### 中转服务

* /etc/wireguard/wg0.conf;
```
[Interface]
PrivateKey = server-private-key
Address = 10.0.2.1/24
ListenPort = 62860

# pi
[Peer]
PublicKey = pi-public-key
PresharedKey = pi-shared-key
AllowedIPs = 10.0.2.2/32
PersistentKeepalive = 25

# st
[Peer]
PublicKey = st-public-key
PresharedKey = st-shared-key
AllowedIPs = 10.0.2.6/32
PersistentKeepalive = 25

# phyllis
[Peer]
PublicKey = phyllis-public-key
PresharedKey = phyllis-shared-key
AllowedIPs = 10.0.2.8/32
PersistentKeepalive = 25
```

### 节点

* 每个节点都有相应的配置,/etc/wireguard/wg0.conf；
```
[Interface]
PrivateKey = pi-private-key
Address = 10.0.2.2/24

[Peer]
PublicKey = server-public-key
PresharedKey = pi-shared-key
Endpoint = wireguard-server-ip:62860
AllowedIPs = 10.0.2.0/24
PersistentKeepalive = 25
```

### Server规则
* 不同节点之间互相通讯需要配置如下规则；
```shell
sudo iptables -I FORWARD 1 -i wg0 -o wg0 -j ACCEPT
```

### 启动

```shell
# 启动
sudo systemctl start wg-quick@wg0

# 查看状态
sudo systemctl status wg-quick@wg0

# 开机启动
sudo systemctl enable wg-quick@wg0

# 取消开机启动
sudo systemctl disable wg-quick@wg0
```

## qbittorrent

### 挂载

* 将远端的/mnt/pxfs/downloads目录挂在到本机/mnt/downloads；
```shell
sshfs st@10.0.0.2:/mnt/pxfs/downloads /mnt/downloads -o allow_other -o max_read=67108864 -o max_write=67108864
```

### docker-compose

```yaml
version: "2.1"
services:
  qbittorrent:
    image: lscr.io/linuxserver/qbittorrent:latest
    container_name: qbittorrent
    environment:
      - PUID=1000
      - PGID=1003
      - TZ=Europe/London
      - WEBUI_PORT=8080
    volumes:
      - /qbt/config:/config
      - /mnt/downloads:/downloads
    ports:
      - 8080:8080
      - 6881:6881
      - 6881:6881/udp
    restart: unless-stopped
```


## v2ray代理wiregurd

* v2ary 配置：
```json
{
    "log": {
        "loglevel": "warning"
    },
    "inbounds": [
        {
            "port": 26800,
            "listen": "127.0.0.1",
            "protocol": "dokodemo-door",
            "settings": {
                "network": "udp",
                "address": "wireguard-server-ip",
                "port": 26800
            },
            "tag": "wireguard"
        }
    ],
    "outbounds": [
        {}
    ]
}
```

* wiregurd 配置:
```json
[Interface]
PrivateKey = pi-private-key
Address = 10.0.6.2/24

[Peer]
PublicKey = server-public-key
PresharedKey = pi-shared-key
Endpoint = 127.0.0.1:26800
AllowedIPs = 10.0.6.0/24
PersistentKeepalive = 25
```
