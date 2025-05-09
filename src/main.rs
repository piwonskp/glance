use std::collections::HashMap;

use tokio::signal::unix::{signal, SignalKind};
use zbus::{zvariant::Value, Connection, Result};
use serde_json::json;
use std::io::Write;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use clap::Parser;


#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
#[command(author = "Piotr Piwoński <piwonskp@gmail.com>", version = env!("CARGO_PKG_VERSION"), about = "A notification server for waybar")]
struct NotificationConfig {
    #[arg(long, default_value = "<b>•</b> [{app}] <b>{summary}</b>: {body}")]
    read_format: String,

    #[arg(long, default_value = "<span color='#00d69e'><b>• [{app}] {summary}: {body}</b></span>")]
    unread_format: String,

    #[arg(long, default_value = "[{app}] <b>{summary}</b>: {body}")]
    bar_format: String,
}

#[derive(Debug, Clone)]
struct Notification {
    app_name: String,
    summary: String,
    body: String,
    read: bool,
}

impl Notification {
    fn format_with(&self, format: &str) -> String {
        format
            .replace("{app}", &self.app_name)
            .replace("{summary}", &self.summary)
            .replace("{body}", &self.body)
    }
}

struct NotificationServer {
    history: IndexMap<u32, Notification>,
    visible_on_bar: Option<usize>,
    last_notification_id: u32,
    config: NotificationConfig,
}

impl NotificationServer {
    fn new() -> Self {
        let config = NotificationConfig::parse();
        Self {
            history: IndexMap::new(),
            visible_on_bar: None,
            last_notification_id: 0,
            config,
        }
    }

    fn add_to_history(&mut self, id: u32, notification: Notification) {
        self.history.insert(id, notification);
    }

    fn get_notification_list(&self) -> String {
        self
            .history
            .iter()
            .rev()
            .map(|(_, notification)| {
                let format = if notification.read {
                    &self.config.read_format
                } else {
                    &self.config.unread_format
                };
                notification.format_with(format)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn bar_text(&self, index: usize) -> String {
        self.history[index].format_with(&self.config.bar_format)
    }

    fn display_notifications_on_bar(&self) {
        let text = if let Some(i) = self.visible_on_bar { &self.bar_text(i) } else { "" };
        let waybar_output = json!({
            "text": text,
            "tooltip": self.get_notification_list(),
        });
        println!("{}", waybar_output);
    }
    
    fn new_notification_display(&self) {
        let text = if let Some(i) = self.visible_on_bar { &self.bar_text(i) } else { "" };
        let waybar_output = json!({
            "text": text,
            "tooltip": self.get_notification_list(),
            "class": "notify"
        });
        println!("{}", waybar_output);
    }

    fn new_id(&mut self) -> u32 {
        if self.last_notification_id == u32::MAX {
            self.last_notification_id = 1;
        } else {
            self.last_notification_id += 1;
        }
        self.last_notification_id
    }

    fn previous_notification(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.visible_on_bar {
            Some(index) if index == 0 => {
                self.mark_read(index);
            }
            Some(index) => {
                self.mark_read(index);
                self.visible_on_bar = Some(index - 1);
            }
            None => {
                // Technically not possible at this point
                // But it makes sense not to display any notification (just display the icon) while keeping the history
                self.visible_on_bar = Some(0);
            }
        }

        self.display_notifications_on_bar();
    }

    fn next_notification(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.visible_on_bar {
            Some(index) if index == self.history.len() - 1 => {
                self.mark_read(index);
            }
            Some(index) => {
                self.mark_read(index);
                self.visible_on_bar = Some(index + 1);
            }
            None => {
                // Technically not possible at this point
                // But it makes sense not to display any notification (just display the icon) while keeping the history
                self.visible_on_bar = Some(0);
            }
        }

        self.display_notifications_on_bar();
    }
    
    fn mark_read(&mut self, index: usize) {
            self.history[index].read = true;
    }

    fn mark_read_and_render(&mut self) {
        if let Some(index) = self.visible_on_bar {
            self.mark_read(index);
        }
        self.display_notifications_on_bar();
    }
}


#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, Value>,
        expire_timeout: i32,
    ) -> u32 {
        let notification = Notification {
            app_name: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            read: false,
        };
        let id = if replaces_id == 0 { self.new_id() } else { replaces_id };
        self.add_to_history(id, notification);
        self.visible_on_bar = Some(self.history.len() - 1);
        self.new_notification_display();

        id
    }

    fn get_capabilities(&self) -> Vec<&str> {
        vec!["body", "actions"]
    }

    fn close_notification(&mut self, id: u32) -> zbus::fdo::Result<()> {
        if id == 0 {
            // Well, that violates spec. Could use signals instead
            // id=0 shouldn't be used anyway according to the spec so it sounds reasonable
            self.history.shift_remove_index(self.visible_on_bar.unwrap());
        } else {
            self.history.shift_remove(&id);
        }
        if self.visible_on_bar >= Some(self.history.len()) {
            self.visible_on_bar = if self.history.len() == 0 { None } else {
                Some(self.history.len() - 1)
            };
        }
        self.display_notifications_on_bar();
        Ok(())
    }

    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        (
            "Glance",
            "Glance",
            env!("CARGO_PKG_VERSION"),
            "1.3",
        )
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    std::io::stdout().flush().unwrap();
    let connection = Connection::session().await?;
    let server = connection.object_server();
    server.at("/org/freedesktop/Notifications", NotificationServer::new()).await?;
    connection.request_name("org.freedesktop.Notifications").await?;

    let sigrtmin = libc::SIGRTMIN();
    let mut signal_mark_read = signal(SignalKind::from_raw(sigrtmin))?;
    let mut signal_previous = signal(SignalKind::from_raw(sigrtmin + 2))?;
    let mut signal_next = signal(SignalKind::from_raw(sigrtmin + 3))?;

    loop {
        tokio::select! {
            _ = signal_mark_read.recv() => {
                if let Ok(server) = server.interface::<_, NotificationServer>("/org/freedesktop/Notifications").await {
                    server.get_mut().await.mark_read_and_render();
                }
            },
            _ = signal_previous.recv() => {
                if let Ok(server) = server.interface::<_, NotificationServer>("/org/freedesktop/Notifications").await {
                    server.get_mut().await.previous_notification();
                }
            },
            _ = signal_next.recv() => {
                if let Ok(server) = server.interface::<_, NotificationServer>("/org/freedesktop/Notifications").await {
                    server.get_mut().await.next_notification();
                }
            },
        }
    }
}
