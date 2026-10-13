#!/bin/sh
# Gives MultiPath its network permissions, so it never needs the password
# prompt. Runs again on every upgrade, as a new binary loses them.
set -e
if [ "$1" = configure ]; then
    setcap cap_net_admin,cap_net_raw+ep /usr/bin/multipath
fi
