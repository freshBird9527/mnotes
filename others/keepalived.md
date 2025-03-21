# keepalived

## build rpm
```shell
yum install -y gcc make autoconf automake openssl-devel \
           libnl3-devel rpmdevtools kmod-devel systemd-devel iptables-devel ipset-devel net-snmp-devel libnftnl-devel libmnl-devel

wget https://keepalived.org/software/keepalived-2.3.2.tar.gz
tar -xvf keepalived-2.3.2.tar.gz
cd keepalived-2.3.2

rpmdev-setuptree
./configure --with-init=systemd
make rpm

ls -l /root/rpmbuild/RPMS/x86_64/
```