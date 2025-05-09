# HeadlessPiPlayer
Headless video player for Raspberry Pi

Required dependencies:
- VLC: sudo apt install libvlc-dev

```sh
ls /usr/local/bin/
sudo cp ./target/release/headless_pi_player /usr/local/bin/
sudo chmod +x /usr/local/bin/headless_pi_player

ls /etc/systemd/system/
sudo nano /etc/systemd/system/headless_pi_player.service
```

```text
[Unit]
Description=Headless Pi Player
After=network.target

[Service]
ExecStart=/usr/local/bin/headless_pi_player
Restart=always
User=root

[Install]
WantedBy=multi-user.target
```

```
Ctrl-x
Y
Enter
```

```sh
sudo systemctl daemon-reexec
sudo systemctl enable headless_pi_player.service
```
//Created symlink /etc/systemd/system/multi-user.target.wants/headless_pi_player.service → /etc/systemd/system/headless_pi_player.service.

```sh
sudo systemctl start headless_pi_player.service
```

```sh
sudo systemctl status headless_pi_player.service
journalctl -u headless_pi_player.service -e
```

Stop until reboot
```sh
sudo systemctl disable headless_pi_player.service
```

Shutdown permamently
```sh
sudo systemctl stop headless_pi_player.service
```
Restart
```sh
sudo systemctl restart headless_pi_player.service
```

Update binary
```sh
sudo systemctl stop headless_pi_player.service
sudo cp ./target/release/headless_pi_player /usr/local/bin/
sudo systemctl start headless_pi_player.service

sudo systemctl status headless_pi_player.service
```


nmcli dev wifi connect "iPhone (Piotr)" password "raspberry"

in VNC before use:
export XDG_RUNTIME_DIR=/run/user/$(id -u)
sudo -E ./target/debug/headless_pi_player
