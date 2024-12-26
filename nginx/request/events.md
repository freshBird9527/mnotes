# events

## accept

### listen_fd

* 初始化listen_fd的读事件为 ngx_event_accept;
* 将listen_fd的读事件加入到epoll(未使能accept_mutex);

```c
static ngx_int_t
ngx_event_process_init(ngx_cycle_t *cycle)
{
    /* 见ngx_http_add_listening 初始化 ls->handler = ngx_http_init_connection*/
    ls = cycle->listening.elts;
    for (i = 0; i < cycle->listening.nelts; i++) {
        c = ngx_get_connection(ls[i].fd, cycle->log);
        c->listening = &ls[i];
        ls[i].connection = c;

        rev = c->read;
        rev->handler = (c->type == SOCK_STREAM) ? ngx_event_accept
                                                : ngx_event_recvmsg;
        ngx_add_event(rev, NGX_READ_EVENT, 0)
    }
}
```

### new_connection

* 初始化新建立的tcp连接为 http_connection;
* 根据是否是ssl和http以及http2初始化读事件回调;
* 写事件回调设置ngx_http_empty_handler，空函数;
* 只将读事件注册到epoll中，写事件不注册；

```c
void
ngx_http_init_connection(ngx_connection_t *c)
{
    ngx_http_connection_t     *hc;

    hc = ngx_pcalloc(c->pool, sizeof(ngx_http_connection_t));
    c->data = hc;

    rev = c->read;
    rev->handler = ngx_http_wait_request_handler;
    c->write->handler = ngx_http_empty_handler;

    ngx_add_timer(rev, cscf->client_header_timeout);
    ngx_handle_read_event(rev, 0);

}
```

## parse


