# phases

## 参考
https://blog.csdn.net/apelife/article/details/54558112

## 11个阶段

```shell
typedef enum {
    NGX_HTTP_POST_READ_PHASE = 0,

    NGX_HTTP_SERVER_REWRITE_PHASE,

    NGX_HTTP_FIND_CONFIG_PHASE,
    NGX_HTTP_REWRITE_PHASE,
    NGX_HTTP_POST_REWRITE_PHASE,

    NGX_HTTP_PREACCESS_PHASE,

    NGX_HTTP_ACCESS_PHASE,
    NGX_HTTP_POST_ACCESS_PHASE,

    NGX_HTTP_PRECONTENT_PHASE,

    NGX_HTTP_CONTENT_PHASE,

    NGX_HTTP_LOG_PHASE
} ngx_http_phases;
```

## 初始化

```c
typedef struct {
    ngx_http_phase_handler_t  *handlers;
    ngx_uint_t                 server_rewrite_index;
    ngx_uint_t                 location_rewrite_index;
} ngx_http_phase_engine_t;
```

```c
static ngx_int_t
ngx_http_init_phase_handlers(ngx_conf_t *cf, ngx_http_core_main_conf_t *cmcf)
{
    ngx_int_t                   j;
    ngx_uint_t                  i, n;
    ngx_uint_t                  find_config_index, use_rewrite, use_access;
    ngx_http_handler_pt        *h;
    ngx_http_phase_handler_t   *ph;
    ngx_http_phase_handler_pt   checker;

    cmcf->phase_engine.server_rewrite_index = (ngx_uint_t) -1;
    cmcf->phase_engine.location_rewrite_index = (ngx_uint_t) -1;
    find_config_index = 0;
    use_rewrite = cmcf->phases[NGX_HTTP_REWRITE_PHASE].handlers.nelts ? 1 : 0;
    use_access = cmcf->phases[NGX_HTTP_ACCESS_PHASE].handlers.nelts ? 1 : 0;

    n = 1                  /* find config phase */
        + use_rewrite      /* post rewrite phase */
        + use_access;      /* post access phase */

    for (i = 0; i < NGX_HTTP_LOG_PHASE; i++) {
        n += cmcf->phases[i].handlers.nelts;
    }

    ph = ngx_pcalloc(cf->pool,
                     n * sizeof(ngx_http_phase_handler_t) + sizeof(void *));
    if (ph == NULL) {
        return NGX_ERROR;
    }

    cmcf->phase_engine.handlers = ph;
    n = 0;

    for (i = 0; i < NGX_HTTP_LOG_PHASE; i++) {
        h = cmcf->phases[i].handlers.elts;

        switch (i) {

        case NGX_HTTP_SERVER_REWRITE_PHASE:
            if (cmcf->phase_engine.server_rewrite_index == (ngx_uint_t) -1) {
                cmcf->phase_engine.server_rewrite_index = n;
            }
            checker = ngx_http_core_rewrite_phase;

            break;

        case NGX_HTTP_FIND_CONFIG_PHASE:
            find_config_index = n;

            ph->checker = ngx_http_core_find_config_phase;
            n++;
            ph++;

            continue;

        case NGX_HTTP_REWRITE_PHASE:
            if (cmcf->phase_engine.location_rewrite_index == (ngx_uint_t) -1) {
                cmcf->phase_engine.location_rewrite_index = n;
            }
            checker = ngx_http_core_rewrite_phase;

            break;

        case NGX_HTTP_POST_REWRITE_PHASE:
            if (use_rewrite) {
                ph->checker = ngx_http_core_post_rewrite_phase;
                ph->next = find_config_index;
                n++;
                ph++;
            }

            continue;

        case NGX_HTTP_ACCESS_PHASE:
            checker = ngx_http_core_access_phase;
            n++;
            break;

        case NGX_HTTP_POST_ACCESS_PHASE:
            if (use_access) {
                ph->checker = ngx_http_core_post_access_phase;
                ph->next = n;
                ph++;
            }

            continue;

        case NGX_HTTP_CONTENT_PHASE:
            checker = ngx_http_core_content_phase;
            break;

        /* NGX_HTTP_POST_READ_PHASE */
        default:
            checker = ngx_http_core_generic_phase;
        }

        n += cmcf->phases[i].handlers.nelts;

        for (j = cmcf->phases[i].handlers.nelts - 1; j >= 0; j--) {
            ph->checker = checker;
            ph->handler = h[j];
            ph->next = n;
            ph++;
        }
    }

    return NGX_OK;
}
```

### phase_engine

#### 无需upstrem

* 配置如下:
```c
    server {
        listen       8080;
        server_name  localhost;

        location / {
            content_by_lua_block {
                ngx.say('Hello,world!')
            }
        }

        error_page   500 502 503 504  /50x.html;
        location = /50x.html {
            root   html;
        }
    }
```

#### 需要upstrem

* 配置如下: 
```c
    upstream backends {
        server 192.168.100.121:8080;
        server 192.168.100.120:8080;
    }

    server {
        listen       8080;
        server_name  localhost;

        location / {
        proxy_pass http://backends;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        }
    }
```

#### 上面两个配置的phase_engine如下

* 可见并未因为是否需要upstream 而产生不同的phase_engine，原因如下:
  * ngx_lua模块初始化阶段执行 clcf->handler = ngx_http_lua_content_handler;
  * proxy模块初始化阶段执行 clcf->handler = ngx_http_proxy_handler;
  * 在 ngx_http_update_location_config (NGX_HTTP_FIND_CONFIG_PHASE) 中执行 r->content_handler = clcf->handler;
  * 在 ngx_http_core_content_phase (NGX_HTTP_CONTENT_PHASE 的checker) 中执行 ngx_http_finalize_request(r, r->content_handler(r));

* 同一个阶段的checker总是同一个，且next都是指向下一个阶段的第一个handler;

```c
cmcf->phase_engine = {
    .server_rewrite_index = 0,
    .location_rewrite_index = 3,
    .handlers = {
        [0] = {
            .checker = ngx_http_core_rewrite_phase,
            .handler = ngx_coolkit_override_method_handler,
            .next = 2,
        },
        [1] = {
            .checker = ngx_http_core_rewrite_phase,
            .handler = ngx_http_rewrite_handler,
            .next = 2,
        },
        [2] = {
            .checker = ngx_http_core_find_config_phase,
            .handler = NULL,
            .next = 0,
        },
        [3] = {
            .checker = ngx_http_core_rewrite_phase,
            .handler = ngx_coolkit_override_method_handler,
            .next = 5,
        },
        [4] = {
            .checker = ngx_http_core_rewrite_phase,
            .handler = ngx_http_rewrite_handler,
            .next = 5,
        },
        [5] = {
            .checker = ngx_http_core_post_rewrite_phase,
            .handler = NULL,
            .next = 2,
        },
        [6] = {
            .checker = ngx_http_core_generic_phase,
            .handler = ngx_http_limit_req_handler,
            .next = 8,
        },
        [7] = {
            .checker = ngx_http_core_generic_phase,
            .handler = ngx_http_limit_conn_handler,
            .next = 8,
        },
        [8] = {
            .checker = ngx_http_core_access_phase,
            .handler = ngx_http_access_handler,
            .next = 11,
        },
        [9] = {
            .checker = ngx_http_core_access_phase,
            .handler = ngx_http_auth_basic_handler,
            .next = 11,
        },
        [10] = {
            .checker = ngx_http_core_post_access_phase,
            .handler = NULL,
            .next = 11,
        }, 
        [11] = {
            .checker = ngx_http_core_generic_phase,
            .handler = ngx_http_try_files_handler,
            .next = 13,
        },
        [12] = {
            .checker = ngx_http_core_generic_phase,
            .handler = ngx_http_mirror_handler,
            .next = 13,
        }
        [13] = {
            .checker = ngx_http_core_content_phase,
            .handler = ngx_http_index_handler,
            .next = 16,
        }
        [14] = {
            .checker = ngx_http_core_content_phase,
            .handler = ngx_http_autoindex_handler,
            .next = 16,
        }
        [15] = {
            .checker = ngx_http_core_content_phase,
            .handler = ngx_http_static_handler,
            .next = 16,
        }
        [16] = {
            .checker = NULL,
            .handler = ngx_coolkit_override_method_handler,
            .next = 0,
        }
    }
}
```

## 请求处理

* 在header解析完毕后会执行该函数，常见events.md;
* 进入该函数时：
  *  c->read->handler == ngx_http_request_handler;
  *  c->write->handler == ngx_http_request_handler;
  *  r->write_event_handlerh == ngx_http_core_run_phases；
  *  r->read_event_handler == ngx_http_block_reading; // 在linux上是空操作
  *  内部请求 r->phase_handler == phase_engine.server_rewrite_index;
  *  非内部请求 r->phase_handler == 0;

* checker 只有返回 `NGX_OK`时才结束，否则继续执行 phase_engine中的handler;

```c
void
ngx_http_core_run_phases(ngx_http_request_t *r)
{
    ngx_int_t                   rc;
    ngx_http_phase_handler_t   *ph;
    ngx_http_core_main_conf_t  *cmcf;

    cmcf = ngx_http_get_module_main_conf(r, ngx_http_core_module);

    ph = cmcf->phase_engine.handlers;

    while (ph[r->phase_handler].checker) {

        rc = ph[r->phase_handler].checker(r, &ph[r->phase_handler]);

        if (rc == NGX_OK) {
            return;
        }
    }
}
```

### generic_checker

* NGX_HTTP_POST_READ_PHASE、使用该checker;
* handler返回 `NGX_OK` 时 跳过当前phase的所有handler准备执行下一个phase;
* handler返回 `NGX_DECLINED` 时 准备执行当前phase的下一个handler，当前handler为当前phase的最后一个时，则准备执行一下一个phase;
* handler返回 `NGX_AGAIN` 或 `NGX_DONE` 时会结束当前事件的处理，参见 `ngx_http_core_run_phases`， 第一次调用时 r->write_event_handlerh == ngx_http_core_run_phases，下一次事件触发继续执行该handler;

```c
ngx_int_t
ngx_http_core_generic_phase(ngx_http_request_t *r, ngx_http_phase_handler_t *ph)
{
    ngx_int_t  rc;

    rc = ph->handler(r);

    if (rc == NGX_OK) {
        r->phase_handler = ph->next;
        return NGX_AGAIN;
    }

    if (rc == NGX_DECLINED) {
        r->phase_handler++;
        return NGX_AGAIN;
    }

    if (rc == NGX_AGAIN || rc == NGX_DONE) {
        return NGX_OK;
    }

    /* rc == NGX_ERROR || rc == NGX_HTTP_...  */

    ngx_http_finalize_request(r, rc);

    return NGX_OK;
}
```

### NGX_HTTP_POST_READ_PHASE

* 当前已知只有 ngx_http_realip_module 模块在该阶段注册了 ngx_http_realip_handler;
* ngx_http_realip_handler 总是返回 `NGX_DECLINED`, 意味着继续执行下一个handler而非进入下一个phase;

### NGX_HTTP_SERVER_REWRITE_PHASE

* ngx_coolkit_module 在该阶段注册 `ngx_coolkit_override_method_handler`;
  * 由 openresty 打包，可以修改请求的method，同时新增了两个变量;

* ngx_http_rewrite_module 在该阶段注册 `ngx_http_rewrite_handler`;
  * 支持break、if、return、set等指令;

* lua-nginx 模块在配置了 server_rewrite_by_lua 指令时注册 ngx_http_lua_server_rewrite_handler;

* 使用 `ngx_http_core_rewrite_phase` 作为checker:
  * handler 返回 `NGX_DECLINED` 时，继续执行下一个handler而非下一个phase的handler;
  * handler 返回 `NGX_DONE` 时，结束当前事件处理且下一次事件触发继续执行当前的handler;

```c
ngx_int_t
ngx_http_core_rewrite_phase(ngx_http_request_t *r, ngx_http_phase_handler_t *ph)
{
    ngx_int_t  rc;

    rc = ph->handler(r);

    if (rc == NGX_DECLINED) {
        r->phase_handler++;
        return NGX_AGAIN;
    }

    if (rc == NGX_DONE) {
        return NGX_OK;
    }

    /* NGX_OK, NGX_AGAIN, NGX_ERROR, NGX_HTTP_...  */

    ngx_http_finalize_request(r, rc);

    return NGX_OK;
}
```

### NGX_HTTP_FIND_CONFIG_PHASE

* 该阶段不能注册handler，因为checker不调用handler;

* 通过uri 查找对应location配置: 
  * 在`ngx_http_core_find_locatio`中会执行`r->loc_conf = node->inclusive->loc_conf`(TODO: 配置分析);
  * `ngx_http_core_find_locatio` 找到配置后返回 NGX_AGAIN;
* 在 `ngx_http_update_location_config`中 执行`r->content_handler = clcf->handler`;
* 判断 请求body是否超过 `client_max_body_size` 限制;
* 找到配置后 r->phase_handler++ 会进入下一个pahse;

```c
ngx_int_t
ngx_http_core_find_config_phase(ngx_http_request_t *r,
    ngx_http_phase_handler_t *ph)
{
    u_char                    *p;
    size_t                     len;
    ngx_int_t                  rc;
    ngx_http_core_loc_conf_t  *clcf;

    r->content_handler = NULL;
    r->uri_changed = 0;

    rc = ngx_http_core_find_location(r);

    if (rc == NGX_ERROR) {
        ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
        return NGX_OK;
    }

    clcf = ngx_http_get_module_loc_conf(r, ngx_http_core_module);

    if (!r->internal && clcf->internal) {
        ngx_http_finalize_request(r, NGX_HTTP_NOT_FOUND);
        return NGX_OK;
    }

    ngx_http_update_location_config(r);

    if (r->headers_in.content_length_n != -1
        && !r->discard_body
        && clcf->client_max_body_size
        && clcf->client_max_body_size < r->headers_in.content_length_n)
    {
        ngx_log_error(NGX_LOG_ERR, r->connection->log, 0,
                      "client intended to send too large body: %O bytes",
                      r->headers_in.content_length_n);

        r->expect_tested = 1;
        (void) ngx_http_discard_request_body(r);
        ngx_http_finalize_request(r, NGX_HTTP_REQUEST_ENTITY_TOO_LARGE);
        return NGX_OK;
    }

    // 一般不会进入该分支
    if (rc == NGX_DONE) {
        ngx_http_clear_location(r);

        r->headers_out.location = ngx_list_push(&r->headers_out.headers);
        if (r->headers_out.location == NULL) {
            ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
            return NGX_OK;
        }

        r->headers_out.location->hash = 1;
        ngx_str_set(&r->headers_out.location->key, "Location");

        if (r->args.len == 0) {
            r->headers_out.location->value = clcf->escaped_name;

        } else {
            len = clcf->escaped_name.len + 1 + r->args.len;
            p = ngx_pnalloc(r->pool, len);

            if (p == NULL) {
                ngx_http_clear_location(r);
                ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
                return NGX_OK;
            }

            r->headers_out.location->value.len = len;
            r->headers_out.location->value.data = p;

            p = ngx_cpymem(p, clcf->escaped_name.data, clcf->escaped_name.len);
            *p++ = '?';
            ngx_memcpy(p, r->args.data, r->args.len);
        }

        ngx_http_finalize_request(r, NGX_HTTP_MOVED_PERMANENTLY);
        return NGX_OK;
    }

    r->phase_handler++;
    return NGX_AGAIN;
}
```
