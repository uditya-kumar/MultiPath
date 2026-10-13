#!/usr/bin/env bash
# A fake network for testing MultiPath on Linux without a second network
# card: "the PC" has two uplinks, each through its own "ISP" (NAT, capped
# at 10 Mbit/s each way), to a small "internet" that answers the health
# probes, DNS, HTTP and iperf3.
#
#   mp-pc  wan1 192.168.11.2 ── 192.168.11.1 mp-isp1 100.64.1.2 ── 100.64.1.1 ┐
#          wan2 192.168.12.2 ── 192.168.12.1 mp-isp2 100.64.2.2 ── 100.64.2.1 ┴ mp-net
#
# mp-net holds 1.1.1.1 and 8.8.8.8 (probes on :443, DNS on :53) and
# 203.0.113.10 (HTTP on :80 with /big.bin, iperf3 on :5201). lab.test
# resolves to 203.0.113.10.
#
# Usage (as root):
#   linux-lab.sh up | down
#   linux-lab.sh pc <command…>      run a command inside the PC
#   linux-lab.sh unplug|plug N      take uplink N's cable out or back in
#   linux-lab.sh blackhole|restore N  cut ISP N off the internet, link stays up
set -euo pipefail

STATE=/tmp/mp-lab
RATE=10mbit
SERVER=203.0.113.10

pc() { ip netns exec mp-pc "$@"; }
isp() { local n=$1; shift; ip netns exec "mp-isp$n" "$@"; }
net() { ip netns exec mp-net "$@"; }

# Starts a long-running service inside a namespace and remembers its pid.
service() {
    local name=$1 ns=$2
    shift 2
    ip netns exec "$ns" setsid "$@" >"$STATE/$name.log" 2>&1 &
    echo $! >"$STATE/$name.pid"
}

up() {
    down 2>/dev/null || true
    mkdir -p "$STATE/www"
    for ns in mp-pc mp-isp1 mp-isp2 mp-net; do
        ip netns add "$ns"
        ip -n "$ns" link set lo up
    done
    # Strict reverse-path filtering, as on Fedora and RHEL: MultiPath has to
    # loosen it, or replies coming in on the uplinks are dropped.
    pc sysctl -qw net.ipv4.conf.all.rp_filter=1 net.ipv4.conf.default.rp_filter=1

    for n in 1 2; do
        ip link add "wan$n" netns mp-pc type veth peer name lan netns "mp-isp$n"
        ip link add uplink netns "mp-isp$n" type veth peer name "isp$n" netns mp-net
        pc ip addr add "192.168.1$n.2/24" dev "wan$n"
        pc ip link set "wan$n" up
        isp "$n" ip addr add "192.168.1$n.1/24" dev lan
        isp "$n" ip addr add "100.64.$n.2/30" dev uplink
        isp "$n" ip link set lan up
        isp "$n" ip link set uplink up
        net ip addr add "100.64.$n.1/30" dev "isp$n"
        net ip link set "isp$n" up

        isp "$n" ip route add default via "100.64.$n.1"
        isp "$n" sysctl -qw net.ipv4.ip_forward=1
        isp "$n" iptables -t nat -A POSTROUTING -o uplink -j MASQUERADE
        # Download towards the PC, and upload from it.
        isp "$n" tc qdisc add dev lan root tbf rate $RATE burst 32kbit latency 100ms
        pc tc qdisc add dev "wan$n" root tbf rate $RATE burst 32kbit latency 100ms
    done
    # Two default routes, like Wi-Fi and a phone both connected.
    pc ip route add default via 192.168.11.1 dev wan1 metric 100
    pc ip route add default via 192.168.12.1 dev wan2 metric 200

    for ip in 1.1.1.1 8.8.8.8 $SERVER; do
        net ip addr add "$ip/32" dev lo
    done
    head -c 64M /dev/urandom >"$STATE/www/big.bin"
    service http mp-net python3 -m http.server 80 --bind 0.0.0.0 --directory "$STATE/www"
    service probe mp-net python3 -m http.server 443 --bind 0.0.0.0 --directory "$STATE/www"
    service iperf mp-net iperf3 -s
    service dns mp-net dnsmasq --keep-in-foreground --no-resolv --no-hosts --user=root \
        --bind-interfaces --listen-address=1.1.1.1,8.8.8.8 --address=/lab.test/$SERVER

    mkdir -p /etc/netns/mp-pc
    echo "nameserver 8.8.8.8" >/etc/netns/mp-pc/resolv.conf
    sleep 1
    echo "lab is up"
}

down() {
    for pid in "$STATE"/*.pid; do
        [ -e "$pid" ] && kill "$(cat "$pid")" 2>/dev/null || true
    done
    for ns in mp-pc mp-isp1 mp-isp2 mp-net; do
        ip netns del "$ns" 2>/dev/null || true
    done
    rm -rf /etc/netns/mp-pc "$STATE"
}

case "${1:-}" in
    up) up ;;
    down) down ;;
    pc) shift; pc "$@" ;;
    unplug) pc ip link set "wan$2" down ;;
    plug)
        pc ip link set "wan$2" up
        pc ip route replace default via "192.168.1$2.1" dev "wan$2" metric "${2}00"
        ;;
    blackhole) isp "$2" iptables -I FORWARD -j DROP ;;
    restore) isp "$2" iptables -D FORWARD -j DROP ;;
    *)
        sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'
        exit 2
        ;;
esac
