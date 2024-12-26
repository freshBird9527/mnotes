# timer

## 使用

### client_header_timeout

* 初始化新accept的连接的调用该函数;

```c
void
ngx_http_init_connection(ngx_connection_t *c)
{
    /* do something */
    rev = c->read;
    rev->handler = ngx_http_wait_request_handler;
    /* do something */

    // 会执行 rev->timer_set = 1;
    ngx_add_timer(rev, cscf->client_header_timeout);

    ngx_handle_read_event(rev, 0);
}
```

* 新建连接可读事件:

```c
static void
ngx_http_wait_request_handler(ngx_event_t *rev)
{
    c = rev->data;

    /* ngx_event_expire_timers 中设置timedout = 1 */
    if (rev->timedout) {
        ngx_log_error(NGX_LOG_INFO, c->log, NGX_ETIMEDOUT, "client timed out");
        ngx_http_close_connection(c);
        return;
    }

    /* do something */
}
```

## 到期

* 在epoll_wait（可能更新 ngx_current_msec）返回后会调用该函数；
* 迭代ngx_event_timer_rbtree 找到所有过期的定时器:
    * 从ngx_event_timer_rbtree删除该事件;
    * 设置ev的定时器相关标志并回调ev->handler(ev);

```c
void
ngx_event_expire_timers(void)
{
    ngx_event_t        *ev;
    ngx_rbtree_node_t  *node, *root, *sentinel;

    sentinel = ngx_event_timer_rbtree.sentinel;

    for ( ;; ) {
        root = ngx_event_timer_rbtree.root;

        if (root == sentinel) {
            return;
        }

        node = ngx_rbtree_min(root, sentinel);

        /* node->key > ngx_current_msec */
        if ((ngx_msec_int_t) (node->key - ngx_current_msec) > 0) {
            return;
        }

        ev = ngx_rbtree_data(node, ngx_event_t, timer);
        ngx_rbtree_delete(&ngx_event_timer_rbtree, &ev->timer);

        // ev->handler 根据这些标志判断是定时器超时还是真的事件到达;
        ev->timer_set = 0;
        ev->timedout = 1;

        ev->handler(ev);
    }
}
```