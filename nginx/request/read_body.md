# request body

## oenresty 读落盘body
```
#user  nobody;
worker_processes  2;

error_log   /tmp/ngx_backend/error.log  error;
pid         /tmp/ngx_backend/nginx.pid;

events {
    worker_connections  102400;
}

http {
    include       /usr/local/openresty/nginx/conf/mime.types;
    default_type  application/octet-stream;

    log_format  main  '$remote_addr - $remote_user [$time_local] "$request" '
                     '$status $body_bytes_sent "$http_referer" '
                     '"$http_user_agent" "$http_x_forwarded_for"';

    access_log  /tmp/ngx_backend/access.log  main;

    sendfile        on;
    #tcp_nopush     on;

    keepalive_timeout  65;
    keepalive_requests 1000;
    #gzip  on;

    client_max_body_size 4096m;
    client_body_buffer_size 1m;
    large_client_header_buffers 4 32k;
    client_body_temp_path client_body_temp 1 2;

    server {
        listen       54321;
        server_name  localhost;

        location / {
            access_by_lua_block {
                local function get_request_body(usock, uri, max)
                    local sock = ngx.socket.tcp()
                    local ok, err = sock:connect(usock)
                    if not ok then
                        return nil, "failed to connect location: " .. err
                    end

                    local subreq = "GET " .. uri .. " HTTP/1.1\r\nHost: internel\r\nAccept: */*\r\n\r\n"
                    local bytes, err = sock:send(subreq)
                    if bytes ~= #subreq then
                        return nil, "failed to send request: " .. err
                    end

                    local header_reader = sock:receiveuntil("\r\n\r\n")
                    local header, err, partial = header_reader()
                    if not header then
                        return nil, "failed to receive response header: " .. err
                    end

                    local body, err, partial = sock:receive(max)
                    if not body then
                        return nil, "failed to receive response body: " .. err
                    end

                    sock:close()
                    return body, nil
                end

                ngx.req.read_body()
                local data = ngx.req.get_body_data()
                if data ~= nil then
                    -- waf_check(body)
                    ngx.say("===== mem body: " .. #data .. " =====")
                    return
                end

                local rpath = ngx.req.get_body_file()
                if rpath == nil then
                    ngx.say("===== no body =====")
                    return
                end

                local tempfile_uri = "/request_body_tempfile" .. rpath
                local body, err = get_request_body("unix:/tmp/ngx_backend/request_tempfile.sock", tempfile_uri, 1024)
                if err ~= nil then
                    ngx.say("===== get body failed, " .. err .. " =====")
                    return
                end

                -- waf_check(body)
                ngx.say("===== file body: " .. #body .. " =====")
                ngx.print(body)
            }
        }
    }

    server {
        listen unix:/tmp/ngx_backend/request_tempfile.sock;
        server_name  internal;
        access_log   off;

        location /request_body_tempfile {
            alias  /;
        }
    }
}
```
