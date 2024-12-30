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

## parse_request

### create_request

* 创建 c->buffer 长度为 client_header_buffer_size;
* 从c中读取数据到c->buffer，当配置有proxy_protocol时尝试解析读取到的数据;
* 设置rev->handler = ngx_http_process_request_line，切换到解析request_line的状态;
* 主动调用一次 ngx_http_process_request_line 尝试解析，因为可能已经接收完request_line;

```c
static void
ngx_http_wait_request_handler(ngx_event_t *rev)
{
    c = rev->data;
    if (rev->timedout) {
        ngx_log_error(NGX_LOG_INFO, c->log, NGX_ETIMEDOUT, "client timed out");
        ngx_http_close_connection(c);
        return;
    }

    if (c->close) {
        ngx_http_close_connection(c);
        return;
    }

    hc = c->data;
    cscf = ngx_http_get_module_srv_conf(hc->conf_ctx, ngx_http_core_module);

    size = cscf->client_header_buffer_size;

    b = c->buffer;
    // 创建 header buffer
    if (b == NULL) {
        b = ngx_create_temp_buf(c->pool, size);
        if (b == NULL) {
            ngx_http_close_connection(c);
            return;
        }

        c->buffer = b;
    }

    // 调用 ngx_unix_recv 从sock中读取数据到c->buffer;
    n = c->recv(c, b->last, size);

    // 省略错误处理
    b->last += n;

    if (hc->proxy_protocol) {
        // 在payload中的protocol
    }

    c->log->action = "reading client request line";

    ngx_reusable_connection(c, 0);

    // 创建request
    c->data = ngx_http_create_request(c);
    rev->handler = ngx_http_process_request_line;
    ngx_http_process_request_line(rev);
}
```

### parse_request_line

* 尝试读取socket中的数据，解析request_line，成功时初始化r相关数据结构;
* 当buffer空间不够时会调用ngx_http_alloc_large_header_buffer分配新空间;
* 当请求行在携带host，这使用该host寻找server配置并用于初始化 r->srv_conf 和 r->loc_conf(会被请求头ngx_http_process_host覆盖);
* 分配r->headers_in.headers，并设置rev->handler = ngx_http_process_request_headers切换到解析request_headers的状态;
* 主动调用一次ngx_http_process_request_headers尝试解析，因为可能已经接收完request_headers;
* 调用 ngx_http_run_posted_requests，触发子请求的 write_event_handler();

```c
static void
ngx_http_process_request_line(ngx_event_t *rev)
{
    c = rev->data;
    r = c->data;

    if (rev->timedout) {
        c->timedout = 1;
        ngx_http_close_request(r, NGX_HTTP_REQUEST_TIME_OUT);
        return;
    }

    rc = NGX_AGAIN;

    for ( ;; ) {

        if (rc == NGX_AGAIN) {
            // 尝试从socket中读取数据，有数据立即返回(c->buffer == r->header_in)
            n = ngx_http_read_request_header(r);

            if (n == NGX_AGAIN || n == NGX_ERROR) {
                break;
            }
        }

        // 尝试解析reuest_line
        rc = ngx_http_parse_request_line(r, r->header_in);

        if (rc == NGX_OK) {

            /* the request line has been parsed successfully */
            r->request_line.len = r->request_end - r->request_start;
            r->request_line.data = r->request_start;
            r->request_length = r->header_in->pos - r->request_start;

            r->method_name.len = r->method_end - r->request_start + 1;
            r->method_name.data = r->request_line.data;

            if (r->http_protocol.data) {
                r->http_protocol.len = r->request_end - r->http_protocol.data;
            }

            if (ngx_http_process_request_uri(r) != NGX_OK) {
                break;
            }

            if (r->schema_end) {
                r->schema.len = r->schema_end - r->schema_start;
                r->schema.data = r->schema_start;
            }

            // host在请求行才能进入该分支，GET http://www.test.com/example HTTP/1.1
            if (r->host_end) {
                host.len = r->host_end - r->host_start;
                host.data = r->host_start;

                rc = ngx_http_validate_host(&host, r->pool, 0);
                /* host 不合法时结束当前请求处理 */

                /* 查找对应的 server {}，并使用其初始化 r->srv_conf 和 r->loc_conf */
                if (ngx_http_set_virtual_server(r, &host) == NGX_ERROR) {
                    break;
                }

                r->headers_in.server = host;
            }

            if (ngx_list_init(&r->headers_in.headers, r->pool, 20,
                              sizeof(ngx_table_elt_t))
                != NGX_OK)
            {
                ngx_http_close_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
                break;
            }

            c->log->action = "reading client request headers";

            // 准备解析 请求头
            rev->handler = ngx_http_process_request_headers;
            ngx_http_process_request_headers(rev);

            break;
        }

        if (rc != NGX_AGAIN) {
            /* 省略错误处理 */
            break;
        }

        /* NGX_AGAIN: a request line parsing is still incomplete */

        if (r->header_in->pos == r->header_in->end) {

            rv = ngx_http_alloc_large_header_buffer(r, 1);

            if (rv == NGX_DECLINED) {
                r->request_line.len = r->header_in->end - r->request_start;
                r->request_line.data = r->request_start;

                ngx_log_error(NGX_LOG_INFO, c->log, 0,
                              "client sent too long URI");
                ngx_http_finalize_request(r, NGX_HTTP_REQUEST_URI_TOO_LARGE);
                break;
            }
        }
    }

    ngx_http_run_posted_requests(c);
}

```

### parse_request_headers

* 调用ngx_http_parse_header_line 逐行解析header；
* 使用解析好header的key去 ngx_http_headers_in 中查询会应得handler，如ngx_http_process_host;
* 所有header解析完毕后调用 ngx_http_process_request_header 做合法性校验如ver小于1.0当携带了host则结束请求、再如请求方法是TRACE则结束请求;
* 此时所有header接收完毕调用 ngx_http_process_request 开始处理请求;

```c
static void
ngx_http_process_request_headers(ngx_event_t *rev)
{
    c = rev->data;
    r = c->data;

    if (rev->timedout) {
        c->timedout = 1;
        ngx_http_close_request(r, NGX_HTTP_REQUEST_TIME_OUT);
        return;
    }

    cmcf = ngx_http_get_module_main_conf(r, ngx_http_core_module);

    rc = NGX_AGAIN;

    for ( ;; ) {

        if (rc == NGX_AGAIN) {
            
            /* 省略申请大header相关逻辑 */
            if (r->header_in->pos == r->header_in->end) {
                rv = ngx_http_alloc_large_header_buffer(r, 0);
            }

            n = ngx_http_read_request_header(r);
            if (n == NGX_AGAIN || n == NGX_ERROR) {
                break;
            }
        }

        /* the host header could change the server configuration context */
        cscf = ngx_http_get_module_srv_conf(r, ngx_http_core_module);

        /* 解析header，根据配置可以允许非法的header*/
        rc = ngx_http_parse_header_line(r, r->header_in,
                                        cscf->underscores_in_headers);

        if (rc == NGX_OK) {
            r->request_length += r->header_in->pos - r->header_name_start;

            if (r->invalid_header && cscf->ignore_invalid_headers) {
                continue;
            }

            /* a header line has been parsed successfully */

            h = ngx_list_push(&r->headers_in.headers);
            if (h == NULL) {
                ngx_http_close_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
                break;
            }

            h->hash = r->header_hash;

            h->key.len = r->header_name_end - r->header_name_start;
            h->key.data = r->header_name_start;
            h->key.data[h->key.len] = '\0';

            h->value.len = r->header_end - r->header_start;
            h->value.data = r->header_start;
            h->value.data[h->value.len] = '\0';

            h->lowcase_key = ngx_pnalloc(r->pool, h->key.len);
            if (h->lowcase_key == NULL) {
                ngx_http_close_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
                break;
            }

            if (h->key.len == r->lowcase_index) {
                ngx_memcpy(h->lowcase_key, r->lowcase_header, h->key.len);

            } else {
                ngx_strlow(h->lowcase_key, h->key.data, h->key.len);
            }

            // 根据key 从 ngx_http_headers_in 找到对应的 ngx_http_header_t，然后调用handler
            hh = ngx_hash_find(&cmcf->headers_in_hash, h->hash,
                               h->lowcase_key, h->key.len);

            // h为host时会找到对应的server并设置r->srv_conf 和 r->loc_conf
            if (hh && hh->handler(r, h, hh->offset) != NGX_OK) {
                break;
            }

            continue;
        }

        if (rc == NGX_HTTP_PARSE_HEADER_DONE) {

            /* a whole header has been parsed successfully */
            r->request_length += r->header_in->pos - r->header_name_start;

            r->http_state = NGX_HTTP_PROCESS_REQUEST_STATE;

            rc = ngx_http_process_request_header(r);

            if (rc != NGX_OK) {
                break;
            }

            ngx_http_process_request(r);

            break;
        }

        if (rc == NGX_AGAIN) {
            continue;
        }

        /* rc == NGX_HTTP_PARSE_INVALID_HEADER */

        ngx_log_error(NGX_LOG_INFO, c->log, 0,
                      "client sent invalid header line: \"%*s\\x%02xd...\"",
                      r->header_end - r->header_name_start,
                      r->header_name_start, *r->header_end);

        ngx_http_finalize_request(r, NGX_HTTP_BAD_REQUEST);
        break;
    }

    ngx_http_run_posted_requests(c);
}
```

## process_request

* 当所有的请求 header 解析完毕后调用该函数;
* 此时c->read->handler还是ngx_http_process_request_headers，c->write->handler还是 ngx_http_empty_handler;
* 移除client_header_timeout定时器，该定时器在 ngx_http_init_connection 创建;
* 设置读写事件的回调函数为 ngx_http_request_handler;
* 调用 ngx_http_handler

```c
void
ngx_http_process_request(ngx_http_request_t *r)
{
    c = r->connection;

#if (NGX_HTTP_SSL)
    /* 省略https相关的逻辑 */
#endif

    /* 移除初始化连接时创建的 client_header_timeout 定时器 */
    if (c->read->timer_set) {
        ngx_del_timer(c->read);
    }

    c->read->handler = ngx_http_request_handler;
    c->write->handler = ngx_http_request_handler;
    r->read_event_handler = ngx_http_block_reading; // 在linux上是空操作，因此即使body到了也不从sock中读取

    ngx_http_handler(r);
}

```

* 非内部请求时根据header的connetion_type设置r->keepalive 标记;
* 此时的 r->write_event_handlerh 还是NULL, 设置为 ngx_http_core_run_phases;
* 调用 ngx_http_core_run_phases 开始过处理请求的 11 个阶段;
  
```c
void
ngx_http_handler(ngx_http_request_t *r)
{
    ngx_http_core_main_conf_t  *cmcf;

    r->connection->log->action = NULL;

    if (!r->internal) {
        switch (r->headers_in.connection_type) {
        case 0:
            r->keepalive = (r->http_version > NGX_HTTP_VERSION_10);
            break;

        case NGX_HTTP_CONNECTION_CLOSE:
            r->keepalive = 0;
            break;

        case NGX_HTTP_CONNECTION_KEEP_ALIVE:
            r->keepalive = 1;
            break;
        }

        r->lingering_close = (r->headers_in.content_length_n > 0
                              || r->headers_in.chunked);
        r->phase_handler = 0;

    } else {
        cmcf = ngx_http_get_module_main_conf(r, ngx_http_core_module);
        r->phase_handler = cmcf->phase_engine.server_rewrite_index;
    }

    r->valid_location = 1;
    r->write_event_handler = ngx_http_core_run_phases;
    ngx_http_core_run_phases(r);
}
```
