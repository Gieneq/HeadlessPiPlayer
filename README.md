# HeadlessPiPlayer
Headless video player for Raspberry Pi

## Setup

**Required dependencies**:
- VLC: sudo apt install libvlc-dev

**Dev dependencies**:
- Rust
  - sudo apt install -y curl build-essential
  - curl https://sh.rustup.rs -sSf | sh
  - . "$HOME/.cargo/env"
  - rustc --version

**Displaing setup**:

HDMI monitor is probably display 0.
(05.2025) RPi4's Rasperry PI OS has by default Waylane displaying technology replacing X. Seems it is not possible to output video started by service to HDMI display - VLC will output video to terminal.

Replaing Waylane with X:

Check available sessions, should be `LXDE-pi-x.desktop`:

```sh
ls /usr/share/xsessions/
```

Edit LightDM config:
```sh
sudo nano /etc/lightdm/lightdm.conf
```

Replace those lines:
```text
[Seat:*]
autologin-user=<username>
autologin-session=LXDE-pi-x
user-session=LXDE-pi-x
```

Rmove stale session files:
```sh
sudo rm -rf /var/lib/lightdm/.Xauthority
sudo rm -rf /home/borsuk/.Xauthority
```

Reboot:

```sh
sudo reboot now
```

**Every session setup**:

Once per session use to be able to display program started in teminal to HDMI display:

```sh
export DISPLAY=:0
export XDG_RUNTIME_DIR=/run/user/$(id -u)
xhost +SI:localuser:$(whoami)
```

Disable screen blanking:

```sh
xset s off          # Disable screen saver
xset -dpms          # Disable DPMS (Energy Star) features
xset s noblank      # Don't blank the video device
```

**WiFi set via CLI**:

nmcli dev wifi connect "<ssid>" password "<psswd>"

**Startup service setup**:

Copy binary and setup service file:

```sh
ls /usr/local/bin/
sudo cp ./target/release/headless_pi_player /usr/local/bin/
sudo chmod +x /usr/local/bin/headless_pi_player

ls /etc/systemd/system/
sudo nano /etc/systemd/system/headless_pi_player.service
```
Copy content of service file, replace user with custom user and check user id:

> Check user id with `id -u <username>`

```text
[Unit]
Description=Headless Pi Player
After=network.target

[Service]
ExecStart=/usr/local/bin/headless_pi_player
Restart=always
User=borsuk
Environment=DISPLAY=:0
Environment=XDG_RUNTIME_DIR=/run/user/1000

[Install]
WantedBy=multi-user.target
```

Save with: ctrl-X -> Y -> Enter

Enable service:

```sh
sudo systemctl daemon-reexec
sudo systemctl enable headless_pi_player.service
```
> There should be output like this:` Created symlink /etc/systemd/system/multi-user.target.wants/headless_pi_player.service → /etc/systemd/system/headless_pi_player.service`.

Start service: 

```sh
sudo systemctl start headless_pi_player.service
```

Check status and logs:

```sh
sudo systemctl status headless_pi_player.service
journalctl -u headless_pi_player.service -e
```

Done! Upon next reboot service will start.

**Utility commands**:

Stop service until next reboot:

```sh
sudo systemctl disable headless_pi_player.service
```

Shutdown service permamently:

```sh
sudo systemctl stop headless_pi_player.service
```

Restart service:

```sh
sudo systemctl restart headless_pi_player.service
```

Update binary proceedure:

```sh
sudo systemctl stop headless_pi_player.service
sudo cp ./target/release/headless_pi_player /usr/local/bin/
sudo systemctl start headless_pi_player.service

sudo systemctl status headless_pi_player.service
```
## Testing

Upload file cmd/curl:

```cmd
curl -F "file=@<path_to_file>"  http://<pi_address>:8080/upload
```