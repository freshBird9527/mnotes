# common-cmd

## docker-compose.yml
```yaml

```

## 管理命令

#### 连接数据：
```shell
# user: app, db: newsletter, host: localhost, port 5432
psql -U app -d newsletter -h localhost -p 5432
```

## 表管理

#### 列出所有表：
```shell
newsletter-> \dt
              List of relations
 Schema |       Name       | Type  |  Owner   
--------+------------------+-------+----------
 public | _sqlx_migrations | table | postgres
 public | subscriptions    | table | postgres
(2 rows)
```

#### 查看表结构：
```shell
newsletter-> \d subscriptions
                       Table "public.subscriptions"
    Column     |           Type           | Collation | Nullable | Default 
---------------+--------------------------+-----------+----------+---------
 id            | uuid                     |           | not null | 
 email         | text                     |           | not null | 
 name          | text                     |           | not null | 
 subscribed_at | timestamp with time zone |           | not null | 
Indexes:
    "subscriptions_pkey" PRIMARY KEY, btree (id)
    "subscriptions_email_key" UNIQUE CONSTRAINT, btree (email)
```