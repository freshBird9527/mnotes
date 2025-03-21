import gdb

def dump_nginx_connections():
    try:
        cycle = gdb.parse_and_eval("ngx_cycle")
    except gdb.error:
        print("Error: Cannot find 'ngx_cycle'. Ensure Nginx is compiled with debug symbols and running.")
        return

    if cycle == 0:
        print("Error: 'ngx_cycle' is NULL.")
        return

    array_size = 10000
    connections = cycle['connections']

    target_ip = "121.22.248.35"
    target_len = 13

    print("Dumping ngx_cycle->connections array...")
    for i in range(array_size):
        conn = connections[i]
        addr = conn['addr_text']
        if int(addr['len']) != target_len:
            continue
        addr_data = gdb.Value(addr['data']).cast(gdb.lookup_type('char').pointer())
        addr_str = addr_data.string(length=target_len, encoding='latin-1')
        if addr_str == target_ip:
            print(f"Index {i}: addr={addr}")

    print("Dump complete.")


class DumpConnections(gdb.Command):
    def __init__(self):
        super(DumpConnections, self).__init__("dump_connections", gdb.COMMAND_USER)

    def invoke(self, arg, from_tty):
        dump_nginx_connections()

DumpConnections()
