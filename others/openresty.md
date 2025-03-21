# openresty使用

## 火焰图
```shell
wget https://github.com/philipp-classen/stapxx/archive/refs/heads/master.zip
```

## wrk压不满

* HEAD方法需要直接返回，不用回body;
* 需要lua_need_request_body，否则post请求在没收完body时就回了200,导致war发完请求后死等响应;

```yaml
version: '3.9'

services:
  openresty:
    image: openresty/openresty:1.21.4.4-focal
    container_name: openresty
    volumes:
      - ./data/nginx.conf:/usr/local/openresty/nginx/conf/nginx.conf
      - ./data/log:/var/log
    ulimits:
      core:
        soft: 0
        hard: 0
    network_mode: host
    restart: always
```

```conf
#user  nobody;
worker_processes  auto;

error_log   error.log  error;
pid         nginx.pid;

events {
    worker_connections  102400;
}


http {
    include       mime.types;
    default_type  application/octet-stream;

    log_format  main  '$remote_addr - $remote_user [$time_local] "$request" '
                     '$status $body_bytes_sent "$http_referer" '
                     '"$http_user_agent" "$http_x_forwarded_for"';

    access_log  off;
    #access_log  access.log  main;

    sendfile        on;
    #tcp_nopush     on;

    keepalive_timeout  65;
    keepalive_requests 10000;
    #gzip  on;

    client_body_buffer_size 2m;
    client_max_body_size 100m;
    large_client_header_buffers 4 64k;

    server {
        listen 9600;
        listen [::]:9600;

        server_name  _;

        location / {
            lua_need_request_body on;

            if ($request_method = HEAD) {
               return 200;
            }

            content_by_lua_block {
                ngx.header["Content-Type"] = "application/json"
                local cli = ngx.var.remote_addr
                local response = string.format([[{"status": "Ok", "node": "0", "cli": "%s"}]], cli)
                ngx.say(response)
            }
        }
    }
}
```

## wrk构造请求

```lua
local cjson = require("cjson")
local cjson_decode = cjson.decode
local table_insert = table.insert
local table_getn = table.getn
local math_random = math.random
local math_randomseed = math.randomseed
local wrk_format = wrk.format

local requests = {}
local requests_cnt = 0


local function read_file(file_path)
   local file = io.open(file_path, "r")
   if file then
       local content = file:read("*a")
       file:close()
       return content
   end
   return nil
end

local function init_requests(file)
   local content = read_file(file)
   local request_items = cjson_decode(content)
   for rid, value in pairs(request_items) do
      value["headers"]["X-Request-Id"] = rid
      value["headers"]["Host"] = "local.allure.fit"
      if #value["body"] > 0 then
          value["headers"]["Content-Length"] = #value["body"]
      else
          value["headers"]["Content-Length"] = 0
          value["body"] = nil
      end

      table_insert(requests, value)
   end
   requests_cnt = table_getn(requests)
end


function request()
   local index = math_random(1, requests_cnt-1)
   local req = requests[index]
   return wrk_format(req["method"], req["path"], req["headers"], req["body"])
end


function init(args)
   init_requests("requests.json")
   math_randomseed(0)
end



```