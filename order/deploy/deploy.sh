#!/bin/sh
# Put the order server on a Debian host (run from this repo):
#   order/deploy/deploy.sh root@HOST PACK_DIR
# PACK_DIR holds the fab pack (vendor/ and the price models it imports); it
# is private and never in this repository. Secrets live only on the host,
# in /etc/fab/env (see order/README.md); this script never writes them.
set -eu
host=${1:?usage: deploy.sh root@HOST PACK_DIR}
pack=${2:?usage: deploy.sh root@HOST PACK_DIR}
here=$(cd "$(dirname "$0")/.." && pwd)
ssh_opts="-o ControlMaster=auto -o ControlPath=$HOME/.ssh/cm-fab-%r@%h -o ControlPersist=10m"
rsync -a --delete --exclude __pycache__ -e "ssh $ssh_opts" "$here/fabsrv/" "$host:/srv/fab/app/fabsrv/"
rsync -a --delete --exclude __pycache__ --exclude cache --exclude data -e "ssh $ssh_opts" "$pack/" "$host:/srv/fab/vendor/"
scp -q $ssh_opts "$here/deploy/fab.service" "$host:/etc/systemd/system/fab.service"
scp -q $ssh_opts "$here/deploy/nginx.conf" "$host:/etc/nginx/sites-available/fab"
scp -q $ssh_opts "$here/deploy/fab-cert.service" "$here/deploy/fab-cert.timer" "$host:/etc/systemd/system/"
ssh $ssh_opts "$host" 'set -e
  chown -R root:root /srv/fab/app /srv/fab/vendor; chmod -R go-w,o-rwx /srv/fab/app /srv/fab/vendor; chgrp -R fab /srv/fab/app /srv/fab/vendor; chmod -R g+rX /srv/fab/app /srv/fab/vendor
  test -f /etc/fab/env || { echo "missing /etc/fab/env"; exit 1; }
  chown root:fab /etc/fab/env; chmod 640 /etc/fab/env
  ln -sf /etc/nginx/sites-available/fab /etc/nginx/sites-enabled/fab; rm -f /etc/nginx/sites-enabled/default
  nginx -t -q && systemctl reload nginx
  # ssh: keys only (as set up), and nothing tunnelled through it
  printf "%s\n" "PasswordAuthentication no" "KbdInteractiveAuthentication no" "PermitRootLogin prohibit-password" "X11Forwarding no" "AllowTcpForwarding no" "AllowAgentForwarding no" "MaxAuthTries 3" "LoginGraceTime 20" > /etc/ssh/sshd_config.d/fab.conf
  sshd -t && systemctl reload ssh
  systemctl daemon-reload && systemctl enable -q fab && systemctl restart fab
  test -f /etc/fab/cert.env && systemctl enable -q --now fab-cert.timer
  sleep 2; curl -s 127.0.0.1:8090/v1/health; echo'
