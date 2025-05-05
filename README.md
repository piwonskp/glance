# Glance
Glance is a lightweight notification daemon written in Rust. It integrates seamlessly with Waybar to display notifications in a non-intrusive manner. Unlike traditional pop-up notifications, Glance focuses on minimal resource usage and distraction-free notifications, helping you maintain focus while staying informed.

## Getting Started

### Prerequisites

- Install [Rust](https://www.rust-lang.org/tools/install).

### Building the Project

First, clone the repository:

```bash
git clone https://github.com/piwonskp/glance.git
cd glance
```

Then, run the following command to build the project:

```bash
cargo build
```

### Integrating with Waybar

To use Glance with Waybar, add the following configuration to your Waybar `config.json` file:

```json
"custom/notification": {
    "exec": "~/dev/glance/target/release/glance",
    "format": "{icon} {text}",
    "format-icons": "",
    "on-scroll-down": "pkill -SIGRTMIN+2 glance",
    "on-scroll-up": "pkill -SIGRTMIN+3 glance",
    "on-click": "pkill -SIGRTMIN glance",
    "on-click-right": "gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification 0",
    "interval": 0,
    "return-type": "json"
}
```

This configuration integrates Glance with Waybar. Here's how it works:

- **`exec`**: Specifies the path to the Glance executable.
- **`format`**: Defines how notifications are displayed, using an icon and text.
- **`format-icons`**: Sets the icon used for notifications.
- **`on-scroll-down`**: Scroll down to move to the next notification (`SIGRTMIN+2` signal).
- **`on-scroll-up`**: Scroll up to move to the previous notification (`SIGRTMIN+3` signal).
- **`on-click`**: Marks the current notification as read (`SIGRTMIN` signal).
- **`on-click-right`**: Closes or deletes the notification using the `org.freedesktop.Notifications.CloseNotification` method.
- **`interval`**: Sets the update interval to 0, meaning it updates only when triggered.
- **`return-type`**: Specifies the return type as JSON for compatibility with Waybar.

Add this configuration to your Waybar `config.json` file and restart Waybar to enable Glance integration.

## FAQ

### Do Not Disturb Mode

To enable a "Do Not Disturb" mode, you can hide Waybar temporarily by sending a signal. Use the following command:

```bash
pkill -USR1 waybar
```

This will toggle the visibility of Waybar, effectively enabling or disabling the "Do Not Disturb" mode.

### Clearing Notification History
Except for closing individual notifications, you might want to clear all notifications at once. Since notifications are stored in memory, restarting Glance will remove them. You can achieve this by either reloading Waybar entirely or restarting the notification module individually.

#### Reload Waybar
To reload Waybar entirely, use the following command:

```bash
pkill -SIGUSR2 waybar
```

#### Restart Notification Module
If you prefer to restart only the notification module without reloading the entire Waybar, update your Waybar configuration to include signal:

```json
"custom/notification": {
    "exec": "~/dev/glance/target/release/glance",
    "format": "{icon} {text}",
    "format-icons": "",
    "on-scroll-down": "pkill -SIGRTMIN+2 glance",
    "on-scroll-up": "pkill -SIGRTMIN+3 glance",
    "on-click": "pkill -SIGRTMIN glance",
    "on-click-right": "gdbus call --session --dest org.freedesktop.Notifications --object-path /org/freedesktop/Notifications --method org.freedesktop.Notifications.CloseNotification 0",
    "signal": 1,
    "interval": 0,
    "return-type": "json"
}
```

To restart only the notification module and clear the notification history, kill glance and send the following signal to Waybar:

```bash
pkill glance && pkill -SIGRTMIN+1 waybar
```

This will execute the `exec` command for the notification module, restarting Glance and clearing the notification history.

### Notification on multiple monitors
Since Waybar currently spawns a separate `exec` process for each module on each monitor, notifications may only appear on a single monitor.


## License

This project is licensed under the MIT License. See the LICENSE file for details.