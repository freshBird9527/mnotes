# load balancing method

## 多态
nginx 支持多种负载均衡方法least_conn、hash、ip_hash、least_conn等，nginx使用多态让uptream依赖接口而不是依赖实现；

* 一个配置实例如下：
```shell
upstream backend {
    ip_hash;

    server backend1.example.com;
    server backend2.example.com;
    server backend3.example.com;
    server backend4.example.com;
}
```

### 接口定义

```c
typedef struct ngx_peer_connection_s  ngx_peer_connection_t;

typedef ngx_int_t (*ngx_event_get_peer_pt)(ngx_peer_connection_t *pc,
    void *data);
typedef void (*ngx_event_free_peer_pt)(ngx_peer_connection_t *pc, void *data,
    ngx_uint_t state);
typedef void (*ngx_event_notify_peer_pt)(ngx_peer_connection_t *pc,
    void *data, ngx_uint_t type);

struct ngx_peer_connection_s {
    ngx_connection_t                *connection;

    // ......
    ngx_event_get_peer_pt            get;     // 类似 trait的method
    ngx_event_free_peer_pt           free;    // 类似 trait的method
    ngx_event_notify_peer_pt         notify;  // 类似 trait的method
    void                            *data;    // 类似 trait object 
    // ......

    NGX_COMPAT_BEGIN(2)
    NGX_COMPAT_END
};
```

### 依赖接口

```c
ngx_int_t
ngx_event_connect_peer(ngx_peer_connection_t *pc)
{
    // ......
    rc = pc->get(pc, pc->data);
    if (rc != NGX_OK) {
        return rc;
    }
    // .....
}
```

### 创建Trait Object


```shell
static ngx_int_t
ngx_http_upstream_init_least_conn_peer(ngx_http_request_t *r,
    ngx_http_upstream_srv_conf_t *us)
{
    // ......
    r->upstream->peer.get = ngx_http_upstream_get_least_conn_peer;

    return NGX_OK;
}


static ngx_int_t
ngx_http_upstream_init_least_conn(ngx_conf_t *cf,
    ngx_http_upstream_srv_conf_t *us)
{
    // ......
    us->peer.init = ngx_http_upstream_init_least_conn_peer;

    return NGX_OK;
}


static ngx_int_t
ngx_http_upstream_init_least_conn(ngx_conf_t *cf,
    ngx_http_upstream_srv_conf_t *us)
{
    // ......

    us->peer.init = ngx_http_upstream_init_least_conn_peer;

    return NGX_OK;
}

// nginx配置指令 least_conn 的回调函数
static char *
ngx_http_upstream_least_conn(ngx_conf_t *cf, ngx_command_t *cmd, void *conf)
{
    ngx_http_upstream_srv_conf_t  *uscf;

    uscf = ngx_http_conf_get_module_srv_conf(cf, ngx_http_upstream_module);

    if (uscf->peer.init_upstream) {
        ngx_conf_log_error(NGX_LOG_WARN, cf, 0,
                           "load balancing method redefined");
    }

    uscf->peer.init_upstream = ngx_http_upstream_init_least_conn;

    uscf->flags = NGX_HTTP_UPSTREAM_CREATE
                  |NGX_HTTP_UPSTREAM_WEIGHT
                  |NGX_HTTP_UPSTREAM_MAX_CONNS
                  |NGX_HTTP_UPSTREAM_MAX_FAILS
                  |NGX_HTTP_UPSTREAM_FAIL_TIMEOUT
                  |NGX_HTTP_UPSTREAM_DOWN
                  |NGX_HTTP_UPSTREAM_BACKUP;

    return NGX_CONF_OK;
}
```


* 可见上面和下面的ngx_http_upstream_peer_t也是多态的一个例子；
```c
typedef struct {
    ngx_http_upstream_init_pt        init_upstream;
    ngx_http_upstream_init_peer_pt   init;
    void                            *data;
} ngx_http_upstream_peer_t;

```
